// update 모듈은 GitHub Releases를 통한 버전 체크 및 셀프 업데이트를 담당합니다.
// reqwest로 GitHub API를 호출하고, 바이너리를 다운받아 현재 실행 파일을 교체합니다.

use std::path::PathBuf;

const REPO: &str = "gigagookbob/rwd";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub API에서 최신 릴리즈 태그를 가져옵니다.
/// env!("CARGO_PKG_VERSION")은 컴파일 시점에 Cargo.toml의 version 값을 문자열로 삽입합니다.
pub async fn check_latest_version() -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "rwd")
        .send()
        .await?
        .json()
        .await?;

    let tag = resp
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or(crate::messages::error::RELEASE_TAG_NOT_FOUND)?;

    // "v0.1.0" → "0.1.0"
    Ok(tag.trim_start_matches('v').to_string())
}

/// 현재 버전보다 새 버전이 있으면 안내 메시지를 출력합니다.
/// 캐시 전략:
/// - 캐시된 latest != current → 이미 업데이트가 있으므로 24시간 TTL 적용 (API 호출 절약)
/// - 캐시된 latest == current → 새 버전이 나왔을 수 있으므로 항상 API 재확인
pub async fn notify_if_update_available() {
    if let Some(cached) = crate::cache::load_update_check() {
        let now = chrono::Utc::now();
        let interval = chrono::Duration::hours(24);
        // 알릴 게 있으면(latest != current) 24시간 캐시 사용
        // 알릴 게 없으면(latest == current) 캐시 무시하고 재확인
        if cached.latest_version != CURRENT_VERSION
            && now - cached.checked_at < interval
        {
            print_update_notice(&cached.latest_version);
            return;
        }
    }

    // 캐시 미스 또는 만료 — GitHub API 호출
    if let Ok(latest) = check_latest_version().await {
        // 결과를 캐시에 저장 (실패해도 조용히 무시)
        let cache = crate::cache::UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: latest.clone(),
        };
        let _ = crate::cache::save_update_check(&cache);

        print_update_notice(&latest);
    }
}

/// 최신 버전이 현재 버전과 다르면 업데이트 안내를 출력합니다.
fn print_update_notice(latest_version: &str) {
    if latest_version != CURRENT_VERSION {
        eprintln!(
            "{}",
            crate::messages::update::new_version_available(latest_version, CURRENT_VERSION)
        );
        eprintln!("{}\n", crate::messages::update::UPDATE_HINT);
    }
}

/// 셀프 업데이트를 수행합니다.
/// 1. GitHub API에서 최신 릴리즈의 바이너리 URL을 가져옴
/// 2. 바이너리를 다운로드
/// 3. 현재 실행 파일을 교체
pub async fn run_update() -> Result<(), Box<dyn std::error::Error>> {
    let latest = check_latest_version().await?;

    if latest == CURRENT_VERSION {
        eprintln!("{}", crate::messages::update::already_latest(CURRENT_VERSION));
        return Ok(());
    }

    eprintln!("{}", crate::messages::update::updating(CURRENT_VERSION, &latest));

    // 플랫폼별 에셋 이름 결정
    let asset_name = detect_asset_name()?;
    let download_url = format!(
        "https://github.com/{REPO}/releases/download/v{latest}/{asset_name}"
    );

    // 바이너리 다운로드
    eprintln!("{}", crate::messages::update::downloading(&download_url));
    let client = reqwest::Client::new();
    let bytes = client
        .get(&download_url)
        .header("User-Agent", "rwd")
        .send()
        .await?
        .bytes()
        .await?;

    // 임시 파일에 저장 후 압축 해제
    let tmp_dir = std::env::temp_dir().join("rwd_update");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir)?;

    let archive_path = tmp_dir.join(&asset_name);
    std::fs::write(&archive_path, &bytes)?;

    // tar.gz 압축 해제 — Unix에서 tar 명령어를 사용합니다.
    let status = std::process::Command::new("tar")
        .args(["-xzf", &archive_path.to_string_lossy(), "-C", &tmp_dir.to_string_lossy()])
        .status()?;

    if !status.success() {
        return Err(crate::messages::error::EXTRACT_FAILED.into());
    }

    // 추출된 바이너리 찾기
    let extracted = find_binary_in_dir(&tmp_dir)?;

    // 현재 실행 파일 교체
    let current_exe = std::env::current_exe()?;
    replace_binary(&extracted, &current_exe)?;

    // 정리
    std::fs::remove_dir_all(&tmp_dir).ok();

    // 업데이트 성공 후 캐시를 갱신하여, 다음 실행 시 "새 버전 있음" 알림이 뜨지 않도록 합니다.
    // latest는 방금 설치한 새 버전이고, 다음 실행 시 CURRENT_VERSION이 이 값과 같아지므로 알림이 스킵됩니다.
    let cache = crate::cache::UpdateCheckCache {
        checked_at: chrono::Utc::now(),
        latest_version: latest.clone(),
    };
    let _ = crate::cache::save_update_check(&cache);

    eprintln!("{}", crate::messages::update::update_complete(&latest));
    Ok(())
}

/// 플랫폼에 맞는 에셋 파일명을 반환합니다.
fn detect_asset_name() -> Result<String, Box<dyn std::error::Error>> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let name = match (os, arch) {
        ("macos", "aarch64") => "rwd-aarch64-apple-darwin.tar.gz",
        ("macos", "x86_64") => "rwd-x86_64-apple-darwin.tar.gz",
        ("linux", "x86_64") => "rwd-x86_64-unknown-linux-gnu.tar.gz",
        _ => return Err(crate::messages::error::unsupported_platform(os, arch).into()),
    };
    Ok(name.to_string())
}

/// 디렉토리에서 rwd 바이너리를 찾습니다.
fn find_binary_in_dir(dir: &std::path::Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // tar.gz이 아닌 rwd로 시작하는 파일이 바이너리
        if name.starts_with("rwd") && !name.ends_with(".tar.gz") && path.is_file() {
            return Ok(path);
        }
    }
    Err(crate::messages::error::BINARY_NOT_FOUND.into())
}

/// 바이너리를 교체합니다.
/// Unix에서는 실행 중인 파일도 교체 가능합니다 (inode 기반 파일 시스템).
fn replace_binary(
    new_binary: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 실행 권한 설정
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new_binary, std::fs::Permissions::from_mode(0o755))?;
    }

    // /usr/local/bin 등에 있으면 권한이 필요할 수 있음
    match std::fs::copy(new_binary, target) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // sudo로 재시도
            eprintln!("{}", crate::messages::error::ADMIN_REQUIRED);
            let status = std::process::Command::new("sudo")
                .args(["cp", &new_binary.to_string_lossy(), &target.to_string_lossy()])
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(crate::messages::error::BINARY_REPLACE_FAILED.into())
            }
        }
        Err(e) => Err(e.into()),
    }
}

use serde::Deserialize;
use std::path::PathBuf;

const OWNER: &str = "nontracey";
const REPO: &str = "AIUsageStatistics";

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
    pub asset_name: String,
}

fn platform_asset_pattern() -> &'static str {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("macos", "aarch64") => "macOS-aarch64",
        ("macos", "x86_64") => "macOS-x86_64",
        ("linux", "x86_64") => "Linux-x86_64",
        ("windows", "x86_64") => "Windows-x86_64",
        _ => "",
    }
}

pub fn check_for_update(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{}/{}/releases/latest", OWNER, REPO);
    let resp = ureq::Agent::new_with_defaults()
        .get(&url)
        .header("User-Agent", "AIUsageStatistics")
        .header("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| format!("请求失败: {}", e))?;

    let body_text = resp.into_body()
        .read_to_string()
        .map_err(|e| format!("读取响应失败: {}", e))?;

    let release: Release = serde_json::from_str(&body_text)
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let latest_tag = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
    let current = current_version.strip_prefix('v').unwrap_or(current_version);

    let latest_ver = semver::Version::parse(latest_tag).map_err(|e| format!("版本号解析失败: {}", e))?;
    let current_ver = semver::Version::parse(current).map_err(|e| format!("版本号解析失败: {}", e))?;

    if latest_ver <= current_ver {
        return Ok(None);
    }

    let pattern = platform_asset_pattern();
    if pattern.is_empty() {
        return Err("不支持的平台".into());
    }

    let asset = release.assets.iter()
        .find(|a| a.name.contains(pattern))
        .ok_or_else(|| format!("未找到 {} 的构建产物", pattern))?;

    Ok(Some(UpdateInfo {
        version: release.tag_name.clone(),
        download_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
    }))
}

pub fn download_and_install(info: &UpdateInfo) -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join("ai-usage-stats-update");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;

    let archive_path = temp_dir.join(&info.asset_name);
    let resp = ureq::Agent::new_with_defaults()
        .get(&info.download_url)
        .call()
        .map_err(|e| format!("下载失败: {}", e))?;

    let mut reader = resp.into_body().into_reader();
    let mut file = std::fs::File::create(&archive_path)
        .map_err(|e| format!("创建文件失败: {}", e))?;
    std::io::copy(&mut reader, &mut file)
        .map_err(|e| format!("写入文件失败: {}", e))?;
    drop(file);

    let extract_dir = temp_dir.join("extracted");
    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir).map_err(|e| format!("清理目录失败: {}", e))?;
    }
    std::fs::create_dir_all(&extract_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    if info.asset_name.ends_with(".zip") {
        let zip_file = std::fs::File::open(&archive_path)
            .map_err(|e| format!("打开压缩包失败: {}", e))?;
        let mut archive = zip::ZipArchive::new(zip_file)
            .map_err(|e| format!("读取压缩包失败: {}", e))?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| format!("读取压缩包条目失败: {}", e))?;
            let out_path = extract_dir.join(file.name());
            if file.is_dir() {
                std::fs::create_dir_all(&out_path).map_err(|e| format!("创建目录失败: {}", e))?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
                }
                let mut outfile = std::fs::File::create(&out_path)
                    .map_err(|e| format!("创建文件失败: {}", e))?;
                std::io::copy(&mut file, &mut outfile)
                    .map_err(|e| format!("解压文件失败: {}", e))?;
            }
        }
    } else {
        let tar_gz = std::fs::File::open(&archive_path)
            .map_err(|e| format!("打开压缩包失败: {}", e))?;
        let decoded = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(decoded);
        archive.unpack(&extract_dir)
            .map_err(|e| format!("解压失败: {}", e))?;
    }

    let binary_name = if cfg!(target_os = "windows") { "ai-usage-statistics.exe" } else { "ai-usage-statistics" };
    let downloaded_bin = extract_dir.join(binary_name);

    if !downloaded_bin.exists() {
        let entries: Vec<_> = match std::fs::read_dir(&extract_dir) {
            Ok(d) => d.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
            Err(_) => vec![],
        };
        let found = entries.iter().find(|p| {
            p.file_name().and_then(|n| n.to_str()).map(|n| n.contains("ai-usage-statistics")).unwrap_or(false)
        }).cloned();
        let bin = found.ok_or_else(|| "解压后未找到可执行文件".to_string())?;
        std::fs::rename(&bin, &downloaded_bin).map_err(|e| format!("重命名失败: {}", e))?;
    }

    Ok(downloaded_bin)
}

pub fn apply_update(new_binary: &std::path::Path) -> Result<(), String> {
    let self_path = std::env::current_exe().map_err(|e| format!("获取自身路径失败: {}", e))?;

    let backup_path = self_path.with_extension("old");
    if backup_path.exists() {
        std::fs::remove_file(&backup_path).map_err(|e| format!("删除旧备份失败: {}", e))?;
    }

    if cfg!(target_os = "windows") {
        let temp_new = std::env::temp_dir().join("ai-usage-stats-update").join("new.exe");
        std::fs::copy(new_binary, &temp_new).map_err(|e| format!("复制到临时目录失败: {}", e))?;
        let batch = format!(
            "@echo off\n\
             timeout /t 1 /nobreak >nul\n\
             copy /y \"{temp}\" \"{target}\"\n\
             start \"\" \"{target}\"\n\
             del \"%~f0\"\n",
            temp = temp_new.display(),
            target = self_path.display()
        );
        let batch_path = std::env::temp_dir().join("update.bat");
        std::fs::write(&batch_path, batch).map_err(|e| format!("写入批处理文件失败: {}", e))?;
        std::process::Command::new("cmd")
            .args(["/c", &batch_path.to_string_lossy()])
            .spawn()
            .map_err(|e| format!("启动更新脚本失败: {}", e))?;
    } else {
        std::fs::rename(&self_path, &backup_path).map_err(|e| format!("备份失败: {}", e))?;
        std::fs::copy(new_binary, &self_path).map_err(|e| {
            let _ = std::fs::rename(&backup_path, &self_path);
            format!("复制新版本失败: {}", e)
        })?;
        #[cfg(unix)]
        std::fs::set_permissions(&self_path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .map_err(|e| format!("设置权限失败: {}", e))?;
    }

    Ok(())
}

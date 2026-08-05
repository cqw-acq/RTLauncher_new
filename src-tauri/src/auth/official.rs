use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlite::{Connection, State};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::sleep;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use tokio::time::Instant;

use super::AccountInfo;

/// 微软登录取消标志：当用户关闭登录对话框时设置为 true
static MS_LOGIN_CANCELLED: AtomicBool = AtomicBool::new(false);

const CLIENT_ID: &str = "1662e9cb-e526-4bea-8237-11526075b7f3";

/// 设备代码信息，返回给前端展示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
    message: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TokenResponse {
    token_type: String,
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct XboxLiveTokenResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Serialize, Deserialize, Debug)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Xui {
    uhs: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct XSTSErrorResponse {
    #[serde(rename = "XErr")]
    x_err: Option<u64>,
    #[serde(rename = "Message")]
    message: Option<String>,
    #[serde(rename = "Redirect")]
    redirect: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MinecraftLoginResponse {
    username: String,
    access_token: String,
    token_type: String,
    expires_in: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
}
/*
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 SQLite 数据库
    let connection = setup_database()?;
    let client = Client::new();
    let client_id = "1662e9cb-e526-4bea-8237-11526075b7f3";

    // 从上到下分别是：添加新账户，检查刷新过期账户，手动下载账号皮肤
    add_new_account(&client, &connection, client_id,).await?;
    check_account_time(&client, &connection, client_id,"Elanda_seaweeds").await?;
    download_player_skin(&client, "6e75722406c4461fb917cf32ace6790c").await?;
    Ok(())
}
*/
async fn get_device_code(
    client: &Client,
    client_id: &str,
) -> Result<DeviceCodeResponse, Box<dyn std::error::Error>> {
    let params = [
        ("client_id", client_id),
        ("scope", "XboxLive.signin offline_access"),
    ];
    let response = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&params)
        .send()
        .await?
        .json::<DeviceCodeResponse>()
        .await?;
    Ok(response)
}

async fn poll_for_token(
    client: &Client,
    client_id: &str,
    device_code: &str,
    interval: u64,
) -> Result<TokenResponse, Box<dyn std::error::Error>> {
    loop {
        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", device_code),
        ];
        let response = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&params)
            .send()
            .await?;
        if response.status().is_success() {
            return Ok(response.json::<TokenResponse>().await?);
        }
        sleep(Duration::from_secs(interval)).await;
    }
}

async fn authenticate_with_xbox_live(
    client: &Client,
    access_token: &str,
) -> Result<XboxLiveTokenResponse, Box<dyn std::error::Error>> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={}", access_token)
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let response = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&body)
        .send()
        .await?
        .json::<XboxLiveTokenResponse>()
        .await?;
    Ok(response)
}

async fn get_xsts_token(
    client: &Client,
    xbox_token: &str,
) -> Result<XboxLiveTokenResponse, Box<dyn std::error::Error>> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbox_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let resp = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        // 尝试解析 XSTS 错误响应
        let xsts_err_msg = if let Ok(err_resp) = serde_json::from_str::<XSTSErrorResponse>(&text) {
            match err_resp.x_err {
                Some(2148916233) => {
                    "该 Microsoft 账户未关联 Xbox 账户，请先前往 xbox.com 注册".to_string()
                }
                Some(2148916235) => "您所在地区不支持 Xbox Live，无法使用正版登录".to_string(),
                Some(2148916236) | Some(2148916237) => "需要在 Xbox 官网完成成人验证".to_string(),
                Some(2148916238) => "未成年账户需要家长在 Microsoft Family 中审批".to_string(),
                Some(code) => format!("XSTS 错误码: {}", code),
                None => err_resp
                    .message
                    .unwrap_or_else(|| format!("HTTP {}: {}", status, text)),
            }
        } else if text.is_empty() {
            format!(
                "XSTS 服务器返回 HTTP {} 且响应体为空，可能是账户权限问题",
                status
            )
        } else {
            format!("HTTP {}: {}", status, text)
        };
        return Err(xsts_err_msg.into());
    }
    let response = serde_json::from_str::<XboxLiveTokenResponse>(&text)
        .map_err(|e| format!("解析 XSTS 响应失败: {} (响应: {})", e, text))?;
    Ok(response)
}

async fn authenticate_with_minecraft(
    client: &Client,
    user_hash: &str,
    xsts_token: &str,
) -> Result<MinecraftLoginResponse, Box<dyn std::error::Error>> {
    let body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={};{}", user_hash, xsts_token)
    });
    let response = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&body)
        .send()
        .await?
        .json::<MinecraftLoginResponse>()
        .await?;
    Ok(response)
}

async fn check_mc_purchase(
    client: &Client,
    access_token: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .bearer_auth(access_token)
        .send()
        .await?;
    if response.status().is_success() {
        let json: serde_json::Value = response.json().await?;
        let items = json.get("items").and_then(|v| v.as_array());
        if items.is_none() || items.unwrap().is_empty() {
            return Ok("您还没有购买mc，请购买后再登录游玩".to_string());
        }
    }
    Ok("您已购买Minecraft".to_string())
}

async fn get_minecraft_profile(
    client: &Client,
    access_token: &str,
) -> Result<MinecraftProfileResponse, Box<dyn std::error::Error>> {
    let response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(access_token)
        .send()
        .await?
        .json::<MinecraftProfileResponse>()
        .await?;
    Ok(response)
}

async fn refresh_access_token(
    client: &Client,
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenResponse, Box<dyn std::error::Error>> {
    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id),
        ("refresh_token", refresh_token),
    ];
    let response = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await?
        .json::<TokenResponse>()
        .await?;
    Ok(response)
}

// 初始化数据库
fn setup_database() -> Result<Connection, Box<dyn std::error::Error>> {
    let connection = sqlite::open(super::db_path())?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS accounts (
            uuid TEXT PRIMARY KEY,
            username TEXT,
            refresh_token TEXT,
            access_token TEXT,
            time INTEGER
        )",
    )?;
    Ok(connection)
}

// 将账户信息保存到数据库
fn save_account_info(
    connection: &Connection,
    username: &str,
    uuid: &str,
    refresh_token: &str,
    access_token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    connection.execute(format!(
        "INSERT OR REPLACE INTO accounts (uuid, username, refresh_token, access_token, time) VALUES ('{}', '{}', '{}', '{}', '{}')",
        uuid, username, refresh_token, access_token, current_time
    ))?;
    Ok(())
}
async fn check_account_time(
    client: &Client,
    connection: &Connection,
    client_id: &str,
    username: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = format!(
        "SELECT uuid, refresh_token, access_token, time FROM accounts WHERE username = '{}'",
        username
    );
    let mut stmt = connection.prepare(query)?;

    if let State::Row = stmt.next()? {
        let uuid: String = stmt.read::<String, _>(0)?;
        let refresh_token: String = stmt.read::<String, _>(1)?;
        let _access_token: String = stmt.read::<String, _>(2)?;
        let last_login_time: i64 = stmt.read::<i64, _>(3)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        if current_time - last_login_time as u64 > 29 * 24 * 3600 {
            // Token is older than 29 days, re-login using device code flow
            println!("Token is older than 29 days, initiating device code flow...");

            let device_code_response = get_device_code(client, client_id).await?;
            println!(
                "Please visit {} and enter code: {}",
                device_code_response.verification_uri, device_code_response.user_code
            );

            let token_response = poll_for_token(
                client,
                client_id,
                &device_code_response.device_code,
                device_code_response.interval,
            )
            .await?;

            let xbox_token_response = authenticate_with_xbox_live(client, &token_response.access_token).await?;
            let xsts_token_response = get_xsts_token(client, &xbox_token_response.token).await?;
            let minecraft_login_response = authenticate_with_minecraft(
                client,
                &xbox_token_response.display_claims.xui[0].uhs,
                &xsts_token_response.token,
            )
            .await?;

            save_account_info(
                connection,
                username,
                &uuid,
                &token_response.refresh_token,
                &minecraft_login_response.access_token,
            )?;

            println!("Device code flow completed. Tokens updated.");
        } else if current_time - last_login_time as u64 > 11 * 3600 {
            // Token is older than 11 hours, refresh it
            println!("Token is older than 11 hours, refreshing access token...");

            let refreshed_token_response =
                refresh_access_token(client, client_id, &refresh_token).await?;

            save_account_info(
                connection,
                username,
                &uuid,
                &refreshed_token_response.refresh_token,
                &refreshed_token_response.access_token,
            )?;

            println!("Access token refreshed.");
        } else {
            println!("Token is still valid.");
        }
    } else {
        println!("No account found with username: {}", username);
    }

    Ok(())
}
async fn download_player_skin(
    client: &Client,
    uuid: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 皮肤存到配置目录下的 skins 子目录
    let profile_dir = format!("{}/skins", super::config_dir());
    fs::create_dir_all(&profile_dir)?;

    // Get player profile to check if skin exists
    let profile_response = client
        .get(&format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{}",
            uuid
        ))
        .send()
        .await?;

    if !profile_response.status().is_success() {
        return Err("Failed to fetch player profile".into());
    }

    let profile_json: serde_json::Value = profile_response.json().await?;
    let properties = profile_json["properties"]
        .as_array()
        .ok_or("No properties found in profile")?;

    // Find the textures property
    let textures_property = properties
        .iter()
        .find(|p| p["name"].as_str() == Some("textures"))
        .ok_or("No textures property found")?;

    // Decode the base64 textures value
    let textures_base64 = textures_property["value"]
        .as_str()
        .ok_or("Textures value is not a string")?;
    let decoded = BASE64.decode(textures_base64)?;
    let textures_json: serde_json::Value = serde_json::from_slice(&decoded)?;

    // Get the skin URL
    let skin_url = textures_json["textures"]["SKIN"]["url"]
        .as_str()
        .ok_or("No skin URL found in textures")?;

    // Download the skin image
    let skin_response = client.get(skin_url).send().await?;
    if !skin_response.status().is_success() {
        return Err("Failed to download skin".into());
    }

    // Save the skin to file
    let skin_bytes = skin_response.bytes().await?;
    let skin_path = format!("{}/{}.png", profile_dir, uuid);
    fs::write(skin_path, skin_bytes)?;

    Ok(())
}

/// 将 UUID 转换为标准格式（带连字符）
fn format_uuid_with_hyphens(uuid: &str) -> String {
    let clean: String = uuid.chars().filter(|c| c.is_alphanumeric()).collect();
    if clean.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &clean[0..8],
            &clean[8..12],
            &clean[12..16],
            &clean[16..20],
            &clean[20..32]
        )
    } else {
        clean
    }
}

/// 将 UUID 转换为无连字符格式
fn format_uuid_without_hyphens(uuid: &str) -> String {
    uuid.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// 从本地数据库获取 tid_skin
fn get_tid_skin_from_database(uuid: &str) -> Option<i64> {
    let db_path = format!("{}/accounts.db", super::config_dir());
    match sqlite::open(&db_path) {
        Ok(conn) => {
            // 尝试带连字符和不带连字符的 UUID
            let uuid_with_hyphens = format_uuid_with_hyphens(uuid);
            let uuid_without_hyphens = format_uuid_without_hyphens(uuid);
            
            // 尝试查询 littleskin_user 表
            for current_uuid in &[uuid, &uuid_with_hyphens, &uuid_without_hyphens] {
                if let Ok(mut stmt) =
                    conn.prepare("SELECT tid_skin FROM littleskin_user WHERE uuid = ?")
                {
                    if let Ok(_) = stmt.bind((1, *current_uuid)) {
                        while let Ok(State::Row) = stmt.next() {
                            if let Ok(tid) = stmt.read::<i64, _>(0) {
                                if tid > 0 {
                                    return Some(tid);
                                }
                            }
                        }
                    }
                }
                
                // 尝试查询 littleskinuser 表（可能是旧表名）
                if let Ok(mut stmt) =
                    conn.prepare("SELECT tid_skin FROM littleskinuser WHERE uuid = ?")
                {
                    if let Ok(_) = stmt.bind((1, *current_uuid)) {
                        while let Ok(State::Row) = stmt.next() {
                            if let Ok(tid) = stmt.read::<i64, _>(0) {
                                if tid > 0 {
                                    return Some(tid);
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        Err(e) => {
            eprintln!("[LittleSkin皮肤] 打开数据库失败: {}", e);
            None
        }
    }
}

/// 从 LittleSkin skinlib 页面 HTML 中提取纹理 URL
fn extract_texture_url_from_html(html: &str) -> Option<String> {
    // 匹配纹理图片的 URL 模式
    if let Ok(texture_re) = Regex::new(
        r#"<img[^>]+src=(['"])(https://textures\.littleskin\.cn/texture/.*?)\1[^>]+class=(['"])skin-preview\2"#,
    ) {
        if let Some(captures) = texture_re.captures(html) {
            return Some(captures[2].to_string());
        }
    }
    
    // 备用模式：查找所有 textures.littleskin.cn 的图片
    if let Ok(fallback_re) = Regex::new(r#"(https://textures\.littleskin\.cn/texture/[^"]+)"#) {
        if let Some(captures) = fallback_re.captures(html) {
            return Some(captures[1].to_string());
        }
    }
    
    None
}

/// 从 textures URL 中提取材质 tid（如 textures.littleskin.cn/texture/12345.png -> 12345）
fn extract_tid_from_texture_url(url: &str) -> Option<i64> {
    // 匹配 /texture/数字.png 或 /raw/数字 等模式
    if let Ok(re) = Regex::new(r#"/(?:texture|raw)/(\d+)(?:\.png)?"#) {
        if let Some(caps) = re.captures(url) {
            if let Some(tid_str) = caps.get(1) {
                return tid_str.as_str().parse::<i64>().ok();
            }
        }
    }
    None
}

/// 将提取到的 tid_skin 保存到数据库（用于后续快速获取皮肤）
fn save_tid_skin_to_database(uuid: &str, tid_skin: i64) {
    let db_path = format!("{}/accounts.db", super::config_dir());
    if let Ok(conn) = sqlite::open(&db_path) {
        let uuid_with_hyphens = format_uuid_with_hyphens(uuid);
        for current_uuid in &[uuid, &uuid_with_hyphens] {
            let query = "UPDATE littleskin_user SET tid_skin = ? WHERE uuid = ?;";
            if let Ok(mut stmt) = conn.prepare(query) {
                let _ = stmt.bind((1, tid_skin));
                let _ = stmt.bind((2, *current_uuid));
                let _ = stmt.next();
            }
        }
    }
}

/// 从给定的 profile JSON 中下载皮肤
fn download_skin_from_profile_json(
    client: &reqwest::blocking::Client,
    profile_json: &serde_json::Value,
    save_path: &str,
    uuid_for_tid_save: Option<&str>,
) -> Result<(), String> {
    let properties = match profile_json["properties"].as_array() {
        Some(p) => p,
        None => return Err("玩家信息中没有 properties".to_string()),
    };

    let textures_property = match properties
        .iter()
        .find(|p| p["name"].as_str() == Some("textures"))
    {
        Some(t) => t,
        None => return Err("没有找到 textures 属性（玩家可能未设置皮肤）".to_string()),
    };

    let textures_base64 = match textures_property["value"].as_str() {
        Some(v) => v,
        None => return Err("textures 值不是字符串".to_string()),
    };

    let decoded = BASE64
        .decode(textures_base64)
        .map_err(|e| format!("base64 解码失败: {}", e))?;

    let textures_json: serde_json::Value =
        serde_json::from_slice(&decoded).map_err(|e| format!("解析 textures JSON 失败: {}", e))?;

    let skin_url = match textures_json["textures"]["SKIN"]["url"].as_str() {
        Some(url) => url.to_string(),
        None => return Err("没有找到皮肤 URL（玩家可能使用默认皮肤）".to_string()),
    };

    // 如果有 UUID，尝试从 skin_url 提取 tid 并保存到数据库（供后续使用 raw API）
    if let Some(uuid) = uuid_for_tid_save {
        if let Some(tid) = extract_tid_from_texture_url(&skin_url) {
            eprintln!(
                "[LittleSkin皮肤] 从 textures URL 提取到 tid_skin: {}, 保存到数据库",
                tid
            );
            save_tid_skin_to_database(uuid, tid);
        }
    }

    // 优先尝试用 raw/<tid> 方式下载（如果能提取到 tid，参考 Blessing Skin 官方文档）
    if let Some(tid) = extract_tid_from_texture_url(&skin_url) {
        let raw_url = format!("https://littleskin.cn/raw/{}", tid);
        eprintln!(
            "[LittleSkin皮肤] textures URL 提取到 tid，优先使用 raw API: {}",
            raw_url
        );
        if let Ok(raw_resp) = client.get(&raw_url).send() {
            if raw_resp.status().is_success() {
                if let Ok(raw_bytes) = raw_resp.bytes() {
                    if raw_bytes.len() >= 8 && raw_bytes.starts_with(&[137, 80, 78, 71]) {
                        fs::write(save_path, raw_bytes)
                            .map_err(|e| format!("保存皮肤文件失败: {}", e))?;
                        return Ok(());
                    }
                }
            }
        }
        eprintln!("[LittleSkin皮肤] raw API 尝试失败，回退到 textures URL 下载");
    }

    let skin_response = client
        .get(&skin_url)
        .send()
        .map_err(|e| format!("下载皮肤失败: {}", e))?;

    if !skin_response.status().is_success() {
        return Err(format!("下载皮肤失败 (HTTP {})", skin_response.status()));
    }

    let skin_bytes = skin_response
        .bytes()
        .map_err(|e| format!("读取皮肤数据失败: {}", e))?;

    fs::write(save_path, skin_bytes).map_err(|e| format!("保存皮肤文件失败: {}", e))?;

    Ok(())
}

/// 通用皮肤下载函数（可用于 Microsoft、LittleSkin、第三方 Yggdrasil）
/// 从指定的 sessionserver URL 获取 textures 并下载皮肤 PNG 到 ./RTL/config/skins/{uuid}.png
///
/// 兼容处理：
///   - 尝试带连字符的 UUID（xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx）
///   - 尝试无连字符的 UUID（32 字符）
///   - 如果 profile 返回的 id 字段与输入不同，优先使用返回的 id 保存
///   - 对于 LittleSkin，增加 sessionserver 路径变体（/sessionserver/session/minecraft/profile/...）
pub fn download_skin_blocking(
    uuid: &str,
    sessionserver_base: &str,
    save_tid_uuid: Option<&str>,
) -> Result<(), String> {
    let profile_dir = format!("{}/skins", super::config_dir());
    fs::create_dir_all(&profile_dir).map_err(|e| format!("创建皮肤目录失败: {}", e))?;

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .danger_accept_invalid_certs(true) // 临时解决证书问题
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());

    let base = sessionserver_base.trim_end_matches('/');
    let uuid_with_hyphens = format_uuid_with_hyphens(uuid);
    let uuid_without_hyphens = format_uuid_without_hyphens(uuid);

    // 尝试多个 URL：标准路径 + Blessing Skin 常见的 /sessionserver/ 前缀变体
    let mut urls = Vec::new();
    for uid in &[&uuid_with_hyphens, &uuid_without_hyphens] {
        urls.push(format!("{}/session/minecraft/profile/{}", base, uid));
        urls.push(format!(
            "{}/sessionserver/session/minecraft/profile/{}",
            base, uid
        ));
    }

    let mut last_error: Option<String> = None;
    let mut profile_json_result: Option<serde_json::Value> = None;

    for url in &urls {
        eprintln!("[通用皮肤下载] 尝试 URL: {}", url);
        match client.get(url).send() {
            Ok(response) => {
                if !response.status().is_success() {
                    last_error = Some(format!("获取玩家信息失败 (HTTP {})", response.status()));
                    continue;
                }
                match response.json::<serde_json::Value>() {
                    Ok(json) => {
                        profile_json_result = Some(json);
                        break;
                    }
                    Err(e) => {
                        last_error = Some(format!("解析玩家信息 JSON 失败: {}", e));
                    }
                }
            }
            Err(e) => {
                last_error = Some(format!("请求玩家信息失败: {}", e));
            }
        }
    }

    let profile_json = match profile_json_result {
        Some(j) => j,
        None => return Err(last_error.unwrap_or_else(|| "无法获取玩家信息".to_string())),
    };

    // 从返回的 profile 中获取 id，优先使用返回的 id 保存
    let saved_uuid = profile_json["id"]
        .as_str()
        .map(|id| format_uuid_with_hyphens(id))
        .unwrap_or_else(|| uuid_with_hyphens.clone());

    let skin_path = format!("{}/{}.png", profile_dir, saved_uuid);

    // 用于保存 tid 的 UUID（优先使用传入的 save_tid_uuid，否则用 saved_uuid）
    let tid_save_uuid = save_tid_uuid.unwrap_or(&saved_uuid);

    // 从 textures 下载皮肤
    match download_skin_from_profile_json(&client, &profile_json, &skin_path, Some(tid_save_uuid)) {
        Ok(()) => Ok(()),
        Err(e) => {
            // 如果失败，同时尝试使用输入的 uuid 保存（某些情况可能保存路径不一致）
            let alt_path = format!("{}/{}.png", profile_dir, uuid_with_hyphens);
            if alt_path != skin_path {
                if let Ok(()) = download_skin_from_profile_json(
                    &client,
                    &profile_json,
                    &alt_path,
                    Some(tid_save_uuid),
                ) {
                    return Ok(());
                }
            }
            Err(e)
        }
    }
}

/// 判断一个字符串是否是玩家名（不是 UUID，不是纯数字 TID）
/// 玩家名规则：长度 3-16，仅含字母、数字、下划线
fn is_minecraft_player_name(s: &str) -> bool {
    s.len() >= 3 && s.len() <= 16 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 判断一个字符串是否是 TID（纯数字）
fn is_tid(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// 判断一个字符串是否是 UUID 格式（带或不带连字符）
fn is_uuid_format(s: &str) -> bool {
    let clean: String = s.chars().filter(|c| c.is_alphanumeric()).collect();
    clean.len() == 32
}

/// 构造带/不带连字符的 UUID 候选列表
fn uuid_candidates(uuid: &str) -> Vec<String> {
    let with_hyphens = format_uuid_with_hyphens(uuid);
    let without_hyphens = format_uuid_without_hyphens(uuid);
    let mut v = Vec::new();
    v.push(uuid.to_string());
    if uuid != with_hyphens {
        v.push(with_hyphens);
    }
    if uuid != without_hyphens {
        v.push(without_hyphens);
    }
    v.dedup();
    v
}

/// 通过 raw/<name_or_tid> 或 skin/<name>.png 等方式尝试下载皮肤（按图片方式处理）
fn try_download_direct(
    client: &reqwest::blocking::Client,
    url: &str,
    save_path: &str,
) -> Result<(), String> {
    let resp = client
        .get(url)
        .send()
        .map_err(|e| format!("请求失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| format!("读取数据失败: {}", e))?;
    if bytes.len() < 8 || !bytes.starts_with(&[137, 80, 78, 71]) {
        return Err("响应不是有效的 PNG".to_string());
    }
    fs::write(save_path, bytes).map_err(|e| format!("保存文件失败: {}", e))?;
    Ok(())
}

/// 通过玩家名下载皮肤（参考 Blessing Skin 官方文档：raw/<玩家名> 是官方推荐的获取完整皮肤方式）
fn download_skin_by_player_name(
    client: &reqwest::blocking::Client,
    player_name: &str,
    save_path: &str,
) -> Result<(), String> {
    // 优先 raw API（官方推荐用于 3D 展示的完整皮肤）
    let raw_url = format!("https://littleskin.cn/raw/{}", player_name);
    eprintln!("[LittleSkin皮肤] 通过玩家名使用 raw API: {}", raw_url);
    match try_download_direct(client, &raw_url, save_path) {
        Ok(()) => return Ok(()),
        Err(e) => eprintln!("[LittleSkin皮肤] raw/{} 失败: {}", player_name, e),
    }
    // 回退: /skin/{name}.png
    let skin_url = format!("https://littleskin.cn/skin/{}.png", player_name);
    eprintln!("[LittleSkin皮肤] 回退使用 skin API: {}", skin_url);
    try_download_direct(client, &skin_url, save_path)
}

/// 通过 TID（材质ID）下载皮肤（参考 Blessing Skin 官方文档：raw/<tid>）
fn download_skin_by_tid(
    client: &reqwest::blocking::Client,
    tid: i64,
    save_path: &str,
) -> Result<(), String> {
    let raw_url = format!("https://littleskin.cn/raw/{}", tid);
    eprintln!("[LittleSkin皮肤] 通过 tid 使用 raw API: {}", raw_url);
    match try_download_direct(client, &raw_url, save_path) {
        Ok(()) => return Ok(()),
        Err(e) => eprintln!("[LittleSkin皮肤] raw/{} 失败: {}", tid, e),
    }
    // 回退: /texture/{tid}.png
    let texture_url = format!("https://littleskin.cn/texture/{}.png", tid);
    eprintln!("[LittleSkin皮肤] 回退使用 texture API: {}", texture_url);
    try_download_direct(client, &texture_url, save_path)
}

/// 专门的 LittleSkin 皮肤下载（因为它可能有特殊的 API 行为）
/// 支持传入可选的 player_name，当传入玩家名时优先通过玩家名方式获取（成功率更高）
pub fn download_littleskin_skin_internal(
    uuid: &str,
    player_name: Option<&str>,
) -> Result<(), String> {
    let littleskin_base = "https://littleskin.cn/api/yggdrasil";
    let cleaned_uuid = format_uuid_with_hyphens(uuid);
    let profile_dir = format!("{}/skins", super::config_dir());
    fs::create_dir_all(&profile_dir).map_err(|e| format!("创建皮肤目录失败: {}", e))?;
    let save_path = format!("{}/{}.png", profile_dir, cleaned_uuid);

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());

    // ---------------------------------------------------------------
    // 情况 A：如果传入的本身就是 TID 或 玩家名（不是 UUID）
    // ---------------------------------------------------------------
    if is_tid(uuid) {
        let tid: i64 = uuid.parse().unwrap();
        eprintln!("[LittleSkin皮肤] 传入值是 TID: {}", tid);
        return download_skin_by_tid(&client, tid, &save_path)
            .map_err(|e| format!("TID方式下载失败: {}", e));
    }
    if is_minecraft_player_name(uuid) {
        eprintln!("[LittleSkin皮肤] 传入值是玩家名: {}", uuid);
        return download_skin_by_player_name(&client, uuid, &save_path)
            .map_err(|e| format!("玩家名方式下载失败: {}", e));
    }

    // ---------------------------------------------------------------
    // 情况 B：传入的是 UUID
    // ---------------------------------------------------------------
    eprintln!(
        "[LittleSkin皮肤] 传入值是 UUID: {}, player_name={:?}",
        cleaned_uuid, player_name
    );

    // 步骤 1：从本地数据库获取 tid_skin（最高优先级）
    for u in &uuid_candidates(uuid) {
        if let Some(tid_skin) = get_tid_skin_from_database(u) {
            eprintln!("[LittleSkin皮肤] 从本地数据库获取到 tid_skin: {}", tid_skin);
            if download_skin_by_tid(&client, tid_skin, &save_path).is_ok() {
                return Ok(());
            }
            break;
        }
    }

    // 步骤 2：如果有玩家名，优先通过玩家名方式获取（成功率最高）
    if let Some(name) = player_name {
        if is_minecraft_player_name(name) {
            eprintln!("[LittleSkin皮肤] 使用传入的玩家名 {} 获取皮肤", name);
            if download_skin_by_player_name(&client, name, &save_path).is_ok() {
                return Ok(());
            }
        }
    }

    // 步骤 3：尝试 LittleSkin 的 sessionserver（标准 Yggdrasil 方式，可以从 textures URL 反提取 tid 保存）
    let mut sessionserver_ok = false;
    for u in &uuid_candidates(uuid) {
        match download_skin_blocking(u, littleskin_base, Some(&cleaned_uuid)) {
            Ok(()) => {
                eprintln!("[LittleSkin皮肤] 从 sessionserver 获取皮肤成功: {}", u);
                sessionserver_ok = true;
                break;
            }
            Err(e) => {
                eprintln!("[LittleSkin皮肤] sessionserver 方式失败 ({}): {}", u, e);
            }
        }
    }
    if sessionserver_ok {
        // 下载成功，检查文件是否在（因为 download_skin_blocking 可能用返回的 id 保存）
        for cand in &uuid_candidates(uuid) {
            let p = format!("{}/{}.png", profile_dir, cand);
            if std::path::Path::new(&p).exists() {
                if cand != &cleaned_uuid {
                    // 如果保存在不同路径，拷贝一份到 cleaned_uuid
                    let _ = fs::copy(&p, &save_path);
                }
                return Ok(());
            }
        }
    }

    // 步骤 4：Fallback：Crafatar 格式（非玩家名/非TID时才尝试，避免把UUID当玩家名）
    let uuid_no_hyphens = format_uuid_without_hyphens(uuid);
    let crafatar_urls = vec![
        format!(
            "https://littleskin.cn/cravatar/skins/{}.png",
            uuid_no_hyphens
        ),
        format!("https://littleskin.cn/cravatar/skins/{}.png", cleaned_uuid),
    ];
    for url in &crafatar_urls {
        eprintln!("[LittleSkin皮肤] 尝试 Crafatar: {}", url);
        if try_download_direct(&client, url, &save_path).is_ok() {
            eprintln!("[LittleSkin皮肤] Crafatar 方式成功");
            return Ok(());
        }
    }

    // 步骤 5：Fallback：如果 player_name 没传，尝试通过 API 用 UUID 反查玩家名，再通过玩家名获取
    // Blessing Skin /api/user ？ 实际很多 Blessing Skin 实例支持 /api/player?uuid=xxx 或 /player
    // 这里尝试用 /api/player/uuid 两种常见路径
    if player_name.is_none() {
        eprintln!("[LittleSkin皮肤] 尝试用 UUID 反查玩家信息");
        let player_api_urls = vec![
            format!("https://littleskin.cn/api/player/{}", uuid_no_hyphens),
            format!("https://littleskin.cn/api/player/{}", cleaned_uuid),
            format!("https://littleskin.cn/player/{}", uuid_no_hyphens),
            format!("https://littleskin.cn/player/{}", cleaned_uuid),
        ];
        for url in &player_api_urls {
            eprintln!("[LittleSkin皮肤] 尝试玩家信息 API: {}", url);
            if let Ok(resp) = client.get(url).send() {
                if resp.status().is_success() {
                    // 先尝试按 JSON 解析
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        // 常见字段: name / player_name / username
                        let extracted_name = json["name"]
                            .as_str()
                            .or_else(|| json["player_name"].as_str())
                            .or_else(|| json["username"].as_str())
                            .or_else(|| json["data"]["name"].as_str())
                            .or_else(|| json["player"]["name"].as_str());
                        if let Some(n) = extracted_name {
                            if is_minecraft_player_name(n) {
                                eprintln!("[LittleSkin皮肤] 反查到玩家名: {}, 尝试下载", n);
                                if download_skin_by_player_name(&client, n, &save_path).is_ok() {
                                    return Ok(());
                                }
                            }
                        }
                        // 尝试从 JSON 中直接取 tid_skin
                        let tid_extracted = json["tid_skin"]
                            .as_i64()
                            .or_else(|| json["skin_tid"].as_i64())
                            .or_else(|| json["data"]["tid_skin"].as_i64());
                        if let Some(tid) = tid_extracted {
                            eprintln!("[LittleSkin皮肤] 从 JSON 中提取到 tid_skin: {}", tid);
                            save_tid_skin_to_database(&cleaned_uuid, tid);
                            if download_skin_by_tid(&client, tid, &save_path).is_ok() {
                                return Ok(());
                            }
                        }
                        // 尝试 textures 字段
                        if let Some(skin_url) =
                            json.get("skin").and_then(|v| v.as_str()).or_else(|| {
                                json.get("textures")
                                    .and_then(|v| v.get("SKIN"))
                                    .and_then(|v| v.get("url"))
                                    .and_then(|v| v.as_str())
                            })
                        {
                            if let Some(tid) = extract_tid_from_texture_url(skin_url) {
                                eprintln!("[LittleSkin皮肤] 从 skin URL 提取 tid: {}", tid);
                                save_tid_skin_to_database(&cleaned_uuid, tid);
                                if download_skin_by_tid(&client, tid, &save_path).is_ok() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let msg = format!("无法从 LittleSkin 获取皮肤 (UUID: {})", cleaned_uuid);
    eprintln!("[LittleSkin皮肤] {}", msg);
    Err(msg)
}

/// 专门的 LittleSkin 皮肤下载（兼容旧接口，无 player_name 参数）
pub fn download_littleskin_skin(uuid: &str) -> Result<(), String> {
    download_littleskin_skin_internal(uuid, None)
}

/// 专门的 LittleSkin 皮肤下载（带 player_name 参数，推荐新代码使用）
pub fn download_littleskin_skin_with_name(uuid: &str, player_name: &str) -> Result<(), String> {
    download_littleskin_skin_internal(uuid, Some(player_name))
}

/// Tauri 命令：重新下载 LittleSkin 皮肤（用于前端皮肤显示失败时刷新）
#[tauri::command]
pub fn redownload_littleskin_skin(uuid: String) -> Result<(), String> {
    eprintln!("[LittleSkin皮肤] 前端请求重新下载皮肤: {}", uuid);
    download_littleskin_skin(&uuid)
}

/// Tauri 命令：读取本地皮肤 PNG 文件，返回 base64（供前端 3D 展示）
///
/// 兼容多种 UUID 格式：优先按传入的 UUID 查找，然后尝试带/不带连字符的格式
#[tauri::command]
pub fn get_skin_base64(uuid: String) -> Result<String, String> {
    let profile_dir = format!("{}/skins", super::config_dir());

    // 生成多个可能的文件名尝试
    let uuid_with_hyphens = format_uuid_with_hyphens(&uuid);
    let uuid_without_hyphens = format_uuid_without_hyphens(&uuid);

    let mut candidate_paths = vec![format!("{}/{}.png", profile_dir, uuid)];
    // 添加带/不带连字符的候选
    if uuid != uuid_with_hyphens {
        candidate_paths.push(format!("{}/{}.png", profile_dir, uuid_with_hyphens));
    }
    if uuid != uuid_without_hyphens {
        candidate_paths.push(format!("{}/{}.png", profile_dir, uuid_without_hyphens));
    }

    let mut last_error: Option<String> = None;
    for path in &candidate_paths {
        eprintln!("[皮肤读取] 尝试路径: {}", path);
        match std::fs::read(path) {
            Ok(bytes) => {
                let b64 = BASE64.encode(&bytes);
                eprintln!("[皮肤读取] 成功读取: {} ({} bytes)", path, bytes.len());
                return Ok(format!("data:image/png;base64,{}", b64));
            }
            Err(e) => {
                last_error = Some(format!("读取皮肤文件失败: {}", e));
                eprintln!("[皮肤读取] 失败 {}: {}", path, e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "皮肤文件不存在".to_string()))
}

/// 删除本地磁盘上的玩家皮肤缓存文件（当账号从 official.rs 数据库中移除时调用，清理残留文件）
/// 会尝试多种 UUID 格式（带/不带连字符），返回成功删除的文件数量
#[tauri::command]
pub fn delete_cached_skin(uuid: String) -> Result<usize, String> {
    let profile_dir = format!("{}/skins", super::config_dir());

    // 生成多个可能的文件名尝试（和 get_skin_base64 保持一致）
    let uuid_with_hyphens = format_uuid_with_hyphens(&uuid);
    let uuid_without_hyphens = format_uuid_without_hyphens(&uuid);

    let mut candidate_paths = vec![format!("{}/{}.png", profile_dir, uuid)];
    if uuid != uuid_with_hyphens {
        candidate_paths.push(format!("{}/{}.png", profile_dir, uuid_with_hyphens));
    }
    if uuid != uuid_without_hyphens {
        candidate_paths.push(format!("{}/{}.png", profile_dir, uuid_without_hyphens));
    }

    let mut deleted_count: usize = 0;
    for path in &candidate_paths {
        match std::fs::remove_file(path) {
            Ok(()) => {
                eprintln!("[正版检测-清理] 已删除残留皮肤文件: {}", path);
                deleted_count += 1;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                // 文件不存在，正常情况，不打印错误
            }
            Err(e) => {
                eprintln!("[正版检测-清理] 删除皮肤文件失败 {}: {}", path, e);
            }
        }
    }

    if deleted_count > 0 {
        eprintln!(
            "[正版检测-清理] UUID={} 的皮肤缓存清理完毕，共删除 {} 个文件",
            uuid, deleted_count
        );
    } else {
        eprintln!(
            "[正版检测-清理] UUID={} 没有找到残留的皮肤文件（可能从未下载过）",
            uuid
        );
    }
    Ok(deleted_count)
}
async fn add_new_account(
    client: &Client,
    connection: &Connection,
    client_id: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    println!("开始新账户登录流程...");

    // 1. 获取设备代码
    let device_code_response = get_device_code(client, client_id).await?;
    println!(
        "请访问 {} 并输入代码: {}",
        device_code_response.verification_uri, device_code_response.user_code
    );

    // 记录开始时间
    let start_time = Instant::now();
    let timeout = Duration::from_secs(300); // 5 分钟超时

    loop {
        // 检查是否超时
        if start_time.elapsed() >= timeout {
            return Err("登录超时".into());
        }

        // 2. 轮询获取token
        let token_response = poll_for_token(
            client,
            client_id,
            &device_code_response.device_code,
            device_code_response.interval,
        )
        .await;

        match token_response {
            Ok(token) => {
                // 3. Xbox Live认证
                let xbox_token_response =
                    authenticate_with_xbox_live(client, &token.access_token).await?;

                // 4. 获取XSTS token
                let xsts_token_response = get_xsts_token(client, &xbox_token_response.token).await?;

                // 5. Minecraft认证
                let minecraft_login_response = authenticate_with_minecraft(
                    client,
                    &xbox_token_response.display_claims.xui[0].uhs,
                    &xsts_token_response.token,
                )
                .await?;

                // 6. 检查是否拥有Minecraft
                let purchase_status =
                    check_mc_purchase(client, &minecraft_login_response.access_token).await?;
                if purchase_status.contains("还没有购买") {
                    return Err(purchase_status.into());
                }

                // 7. 获取Minecraft个人资料
                let profile =
                    get_minecraft_profile(client, &minecraft_login_response.access_token).await?;

                // 8. 下载玩家皮肤
                download_player_skin(client, &profile.id).await?;

                // 9. 保存账户信息到数据库
                save_account_info(
                    connection,
                    &profile.name,
                    &profile.id,
                    &token.refresh_token,
                    &minecraft_login_response.access_token,
                )?;

                // 返回用户名和UUID
                return Ok((profile.name, profile.id));
            }
            Err(_) => {
                // 如果未成功获取token，继续等待
                sleep(Duration::from_secs(device_code_response.interval)).await;
            }
        }
    }
}

// ======================== Tauri Commands ========================

/// 构造带连接超时和读取超时的 HTTP 客户端，避免断网时无限卡死
fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("无法创建 HTTP 客户端: {}", e))
}

/// 将 reqwest 网络错误转换为对用户友好的中文提示
fn friendly_net_err(e: impl std::fmt::Display) -> String {
    let msg = e.to_string();
    if msg.contains("connect")
        || msg.contains("connection")
        || msg.contains("timed out")
        || msg.contains("timeout")
        || msg.contains("dns")
        || msg.contains("resolve")
    {
        format!("网络连接失败，请检查您的网络后重试（{}）", msg)
    } else {
        msg
    }
}

/// 第一步：请求设备代码，前端展示 user_code 和 verification_uri
#[tauri::command]
pub async fn ms_request_device_code() -> Result<DeviceCodeInfo, String> {
    eprintln!("[正版检测] 步骤4：⟡ 进入强制重新登录流程！请求设备授权码...");
    // 重置取消标志（新的一次登录流程开始）
    MS_LOGIN_CANCELLED.store(false, Ordering::SeqCst);
    let client = build_http_client()?;
    let resp = get_device_code(&client, CLIENT_ID)
        .await
        .map_err(|e| friendly_net_err(e))?;
    eprintln!(
        "[正版检测] 步骤4：✓ 设备码获取成功！user_code={}, verification_uri={}, 有效期={}秒",
        resp.user_code, resp.verification_uri, resp.expires_in
    );
    // 自动打开浏览器让用户授权
    let _ = webbrowser::open(&resp.verification_uri);
    Ok(DeviceCodeInfo {
        user_code: resp.user_code,
        verification_uri: resp.verification_uri,
        device_code: resp.device_code,
        interval: resp.interval,
        expires_in: resp.expires_in,
    })
}

/// 第二步：轮询等待用户授权，完成后走完整认证链并返回 AccountInfo
#[tauri::command]
pub async fn ms_poll_and_login(device_code: String, interval: u64) -> Result<AccountInfo, String> {
    eprintln!("[正版检测] 步骤4：开始轮询微软登录授权（间隔={}秒，超时=300秒）", interval);
    let client = build_http_client()?;
    let start_time = Instant::now();
    let timeout = Duration::from_secs(300); // 5 分钟超时
    let mut poll_count: u32 = 0;

    loop {
        // 检查用户是否已取消登录
        if MS_LOGIN_CANCELLED.load(Ordering::SeqCst) {
            eprintln!("[正版检测] 步骤4：✗ 用户取消了登录");
            return Err("已取消登录".to_string());
        }

        if start_time.elapsed() >= timeout {
            eprintln!("[正版检测] 步骤4：✗ 登录超时（超过5分钟）");
            return Err("登录超时，请重试".to_string());
        }

        sleep(Duration::from_secs(interval)).await;

        // sleep 之后再次检查取消状态
        if MS_LOGIN_CANCELLED.load(Ordering::SeqCst) {
            eprintln!("[正版检测] 步骤4：✗ 用户取消了登录");
            return Err("已取消登录".to_string());
        }

        poll_count += 1;
        if poll_count % 10 == 1 {
            eprintln!("[正版检测] 步骤4：轮询中... 第{}次请求（已等待约{}秒）", poll_count, poll_count * interval as u32);
        }

        // 单次轮询尝试
        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", CLIENT_ID),
            ("device_code", device_code.as_str()),
        ];
        let response = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| friendly_net_err(e))?;

        if !response.status().is_success() {
            continue; // 用户尚未授权，继续轮询
        }

        eprintln!("[正版检测] 步骤4：✓ 用户已授权！Microsoft Token 获取成功（第{}次轮询）", poll_count);

        let token: TokenResponse = response
            .json()
            .await
            .map_err(|e| format!("解析 Token 失败: {}", e))?;

        // Xbox Live 认证
        eprintln!("[正版检测] 步骤4：正在进行 Xbox Live 认证...");
        let xbox = authenticate_with_xbox_live(&client, &token.access_token)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect")
                    || msg.contains("timed out")
                    || msg.contains("timeout")
                    || msg.contains("dns")
                    || msg.contains("resolve")
                {
                    format!("网络连接失败，请检查您的网络后重试（{}）", msg)
                } else {
                    format!("Xbox Live 认证失败: {}", msg)
                }
            })?;
        eprintln!("[正版检测] 步骤4：✓ Xbox Live 认证成功");

        // XSTS Token
        eprintln!("[正版检测] 步骤4：正在获取 XSTS Token...");
        let xsts = get_xsts_token(&client, &xbox.token)
            .await
            .map_err(|e| format!("XSTS 认证失败: {}", e))?;
        eprintln!("[正版检测] 步骤4：✓ XSTS Token 获取成功");

        // Minecraft 认证
        let uhs = xbox.display_claims.xui.first()
            .map(|x| x.uhs.clone())
            .ok_or_else(|| "Xbox Live 认证返回的 xui 为空".to_string())?;
        eprintln!("[正版检测] 步骤4：正在进行 Minecraft 服务端认证...");
        let mc_login = authenticate_with_minecraft(
            &client,
            &uhs,
            &xsts.token,
        )
        .await
        .map_err(|e| format!("Minecraft 认证失败: {}", e))?;
        eprintln!("[正版检测] 步骤4：✓ Minecraft 认证成功");

        // 检查是否拥有 Minecraft
        eprintln!("[正版检测] 步骤4：正在检查 Minecraft 购买状态...");
        let purchase = check_mc_purchase(&client, &mc_login.access_token)
            .await
            .map_err(|e| format!("检查购买状态失败: {}", e))?;
        if purchase.contains("还没有购买") {
            eprintln!("[正版检测] 步骤4：✗ 该微软账号尚未购买 Minecraft");
            return Err(purchase);
        }
        eprintln!("[正版检测] 步骤4：✓ 购买状态正常");

        // 获取 Minecraft 个人资料
        eprintln!("[正版检测] 步骤4：正在获取 Minecraft 个人资料...");
        let profile = get_minecraft_profile(&client, &mc_login.access_token)
            .await
            .map_err(|e| format!("获取 Minecraft 资料失败: {}", e))?;
        eprintln!("[正版检测] 步骤4：✓ 获取个人资料成功！玩家名={}, UUID={}", profile.name, profile.id);

        // 构造返回值（先于数据库/皮肤操作，确保即使后续失败也能返回）
        let account_info = AccountInfo {
            name: profile.name.clone(),
            uuid: profile.id.clone(),
            auth_type: "microsoft".to_string(),
            access_token: mc_login.access_token.clone(),
            skin_url: Some(profile.id.clone()),
        };

        // 保存到数据库（非致命，通过 spawn_blocking 隔离 sqlite 线程安全问题）
        let db_name = profile.name.clone();
        let db_id = profile.id.clone();
        let db_refresh = token.refresh_token.clone();
        let db_access = mc_login.access_token.clone();
        let db_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let connection = setup_database().map_err(|e| e.to_string())?;
            save_account_info(&connection, &db_name, &db_id, &db_refresh, &db_access)
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await;
        match db_result {
            Ok(Ok(())) => eprintln!("[正版检测] 步骤4：✓ 账号信息已保存到数据库"),
            Ok(Err(e)) => eprintln!("[MS登录] 数据库保存失败(非致命): {}", e),
            Err(e) => eprintln!("[MS登录] 数据库任务崩溃(非致命): {}", e),
        }

        // 下载皮肤（非致命）
        match download_player_skin(&client, &profile.id).await {
            Ok(()) => eprintln!("[正版检测] 步骤4：✓ 玩家皮肤下载完成"),
            Err(e) => eprintln!("[MS登录] 皮肤下载失败(非致命): {}", e),
        }

        eprintln!("[正版检测] 步骤4：✅ 强制重新登录流程完成！玩家={} 登录成功", profile.name);
        return Ok(account_info);
    }
}

/// 用户关闭登录对话框时调用：中止后台的轮询循环
#[tauri::command]
pub fn ms_cancel_login() -> Result<(), String> {
    MS_LOGIN_CANCELLED.store(true, Ordering::SeqCst);
    Ok(())
}

// ============== 皮肤/披风管理（基于 Minecraft Services API） ==============

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MCSkinInfo {
    pub id: String,
    pub state: String, // ACTIVE / INACTIVE
    pub url: String,
    pub variant: String, // classic / slim
    pub alias: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MCCapeInfo {
    pub id: String,
    pub state: String, // ACTIVE / INACTIVE
    pub url: String,
    pub alias: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MCSkinCapeProfile {
    pub skins: Vec<MCSkinInfo>,
    pub capes: Vec<MCCapeInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MCFullProfileResponse {
    id: String,
    name: String,
    skins: Option<Vec<MCFullSkin>>,
    capes: Option<Vec<MCFullCape>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MCFullSkin {
    id: String,
    state: String,
    url: String,
    variant: Option<String>,
    alias: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MCFullCape {
    id: String,
    state: String,
    url: String,
    alias: Option<String>,
}

/// 获取 Minecraft 完整资料（皮肤列表 + 披风列表）
#[tauri::command]
pub async fn ms_get_skins_and_capes(access_token: String) -> Result<MCSkinCapeProfile, String> {
    eprintln!("[正版检测] 步骤2：调用皮肤披风模块试探 access_token 有效性");
    if access_token.trim().is_empty() {
        eprintln!("[正版检测] 步骤2：✗ access_token 为空字符串！直接返回失败");
        return Err("账户 access_token 不存在，请重新登录".to_string());
    }
    // 打印 token 的前8位和后4位，方便调试但不泄露完整 token
    let token_len = access_token.len();
    let token_preview = if token_len >= 12 {
        format!(
            "{}...{} (长度={})",
            &access_token[0..8],
            &access_token[token_len - 4..],
            token_len
        )
    } else {
        format!("(长度={})", token_len)
    };
    eprintln!("[正版检测] 步骤2：正在请求 Minecraft Services API，token={}", token_preview);

    let client = build_http_client()?;
    let resp = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| {
            let err = friendly_net_err(e);
            eprintln!("[正版检测] 步骤2：✗ 网络请求失败 -> {}", err);
            err
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_msg = format!("获取皮肤资料失败 (HTTP {})", status);
        if status.as_u16() == 401 {
            eprintln!("[正版检测] 步骤2：✗ HTTP 401 Unauthorized -> access_token 已过期或无效，需要重新登录");
        } else {
            eprintln!("[正版检测] 步骤2：✗ HTTP {} -> {}", status, err_msg);
        }
        return Err(err_msg);
    }

    let profile: MCFullProfileResponse = resp
        .json()
        .await
        .map_err(|e| format!("解析皮肤资料失败: {}", e))?;

    let skins_count = profile.skins.as_ref().map(|s| s.len()).unwrap_or(0);
    let capes_count = profile.capes.as_ref().map(|c| c.len()).unwrap_or(0);
    eprintln!(
        "[正版检测] 步骤2：✓ 皮肤披风获取成功！玩家={}, 皮肤数={}, 披风数={} -> 账号正常，无需重新登录",
        profile.name, skins_count, capes_count
    );

    let skins: Vec<MCSkinInfo> = profile
        .skins
        .unwrap_or_default()
        .into_iter()
        .map(|s| MCSkinInfo {
            id: s.id,
            state: s.state,
            url: s.url,
            variant: s.variant.unwrap_or_else(|| "classic".to_string()),
            alias: s.alias,
        })
        .collect();

    let capes: Vec<MCCapeInfo> = profile
        .capes
        .unwrap_or_default()
        .into_iter()
        .map(|c| MCCapeInfo {
            id: c.id,
            state: c.state,
            url: c.url,
            alias: c.alias,
        })
        .collect();

    Ok(MCSkinCapeProfile { skins, capes })
}

/// 上传新皮肤（PNG base64）并设置为当前皮肤
/// variant: "classic" 或 "slim"
#[tauri::command]
pub async fn ms_upload_skin(
    access_token: String,
    png_base64: String,
    variant: String,
) -> Result<String, String> {
    use base64::Engine as _;

    let client = build_http_client()?;

    // 解码 base64 -> 原始 PNG 字节
    let raw_png = BASE64
        .decode(png_base64.trim())
        .map_err(|e| format!("皮肤 base64 解码失败: {}", e))?;

    // ── 手动构造 multipart/form-data（不依赖 reqwest multipart feature）
    let boundary = "----RTLauncherSkinBoundaryXYZ123456";
    let mut body: Vec<u8> = Vec::new();

    // 第一部分：file (PNG 图片)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    body.extend_from_slice(&raw_png);
    body.extend_from_slice(b"\r\n");

    // 第二部分：variant
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"variant\"\r\n\r\n");
    body.extend_from_slice(variant.as_bytes());
    body.extend_from_slice(b"\r\n");

    // 结束标记
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let resp = client
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(&access_token)
        .header(
            reqwest::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .send()
        .await
        .map_err(|e| friendly_net_err(e))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("上传皮肤失败 (HTTP {}): {}", status, text));
    }

    // 上传成功后，从响应中解析并返回新皮肤 ID
    let profile: MCFullProfileResponse = resp
        .json()
        .await
        .map_err(|e| format!("解析上传响应失败: {}", e))?;

    // 找到新上传的皮肤（通常第一个 ACTIVE 就是新上传的）
    let new_skin_id = profile
        .skins
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.state == "ACTIVE")
        .map(|s| s.id)
        .unwrap_or_else(|| "unknown".to_string());

    // 上传皮肤成功后，下载到本地（供 3D 展示）
    let uuid = profile.id.clone();
    let _ = download_skin_blocking(&uuid, "https://sessionserver.mojang.com", None);

    Ok(new_skin_id)
}

/// 激活指定皮肤（从已有皮肤列表中切换）
#[tauri::command]
pub async fn ms_activate_skin(
    access_token: String,
    skin_id: String,
    variant: String,
) -> Result<(), String> {
    let client = build_http_client()?;
    let body = serde_json::json!({ "variant": variant });
    let resp = client
        .put(&format!(
            "https://api.minecraftservices.com/minecraft/profile/skins/{}",
            skin_id
        ))
        .bearer_auth(&access_token)
        .json(&body)
        .send()
        .await
        .map_err(|e| friendly_net_err(e))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("切换皮肤失败 (HTTP {}): {}", status, text));
    }
    Ok(())
}

/// 删除指定皮肤
#[tauri::command]
pub async fn ms_delete_skin(access_token: String, skin_id: String) -> Result<(), String> {
    let client = build_http_client()?;
    let resp = client
        .delete(&format!(
            "https://api.minecraftservices.com/minecraft/profile/skins/{}",
            skin_id
        ))
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| friendly_net_err(e))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("删除皮肤失败 (HTTP {}): {}", status, text));
    }
    Ok(())
}

/// 设置激活披风（capeId 为空则取消激活）
#[tauri::command]
pub async fn ms_set_active_cape(access_token: String, cape_id: String) -> Result<(), String> {
    let client = build_http_client()?;

    let resp = if cape_id.is_empty() {
        // 取消激活披风
        client
            .delete("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| friendly_net_err(e))?
    } else {
        // 设置激活披风
        let body = serde_json::json!({ "capeId": cape_id });
        client
            .put("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .bearer_auth(&access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| friendly_net_err(e))?
    };

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("设置披风失败 (HTTP {}): {}", status, text));
    }
    Ok(())
}

/// 尝试通过数据库中的 refresh_token 静默刷新微软账号
/// 成功：返回更新后的 AccountInfo（包含新的 access_token、name、uuid、skin_url）
/// 失败：返回错误（例如 "NO_REFRESH_TOKEN"、"REFRESH_FAILED" 等），表示必须重新走设备码授权流程
#[tauri::command]
pub async fn ms_silent_refresh_account(uuid: String) -> Result<AccountInfo, String> {
    eprintln!("[正版检测] 步骤3：尝试用 refresh_token 静默刷新账号 UUID={}", uuid);
    // ── 第一步：从数据库中取出 refresh_token（同步块内完成，避免非 Send 的 SQLite Statement/Connection 跨 await）──
    let (db_uuid, db_refresh_token): (String, String) = {
        let connection = setup_database().map_err(|e| format!("打开账户数据库失败: {}", e))?;
        let query = format!(
            "SELECT uuid, username, refresh_token, access_token, time FROM accounts WHERE uuid = '{}'",
            uuid.replace('\'', "''")
        );
        let mut stmt = connection.prepare(query).map_err(|e| e.to_string())?;

        if let State::Row = stmt.next().map_err(|e| e.to_string())? {
            let u: String = stmt.read::<String, _>(0).map_err(|e| e.to_string())?;
            let n: String = stmt.read::<String, _>(1).map_err(|e| e.to_string())?;
            let rt: String = stmt.read::<String, _>(2).map_err(|e| e.to_string())?;
            if rt.trim().is_empty() {
                eprintln!("[正版检测] 步骤3：✗ 数据库中 refresh_token 为空，无法静默刷新");
                return Err("NO_REFRESH_TOKEN".to_string());
            }
            eprintln!("[正版检测] 步骤3：从数据库读取到玩家名={}, refresh_token 存在 (长度={})", n, rt.len());
            (u, rt)
        } else {
            eprintln!("[正版检测] 步骤3：✗ 数据库中没有该 UUID 的账号记录，无法静默刷新");
            return Err("NO_REFRESH_TOKEN".to_string());
        }
        // stmt、connection 在这里离开作用域被 drop，不会保留到 await 之后
    };

    // ── 第二步：HTTP / await 部分（作用域中不再保留任何 db 相关对象）──
    let client = build_http_client().map_err(|e| e.to_string())?;

    eprintln!("[正版检测] 步骤3：正在请求 Microsoft Token 刷新接口...");
    // 2) 调用 Microsoft token endpoint 刷新 access_token
    let refreshed = refresh_access_token(&client, CLIENT_ID, &db_refresh_token)
        .await
        .map_err(|_| {
            eprintln!("[正版检测] 步骤3：✗ refresh_token 已失效，Microsoft Token 接口返回失败");
            "REFRESH_FAILED".to_string()
        })?;
    eprintln!("[正版检测] 步骤3：✓ Microsoft Token 刷新成功，新 access_token 长度={}", refreshed.access_token.len());

    eprintln!("[正版检测] 步骤3：正在走 Xbox Live / XSTS / Minecraft 完整认证链...");
    // 3) 走一遍 Xbox Live / XSTS / Minecraft 登录链条，拿到 Minecraft access_token
    let xbox_token = authenticate_with_xbox_live(&client, &refreshed.access_token)
        .await
        .map_err(|_| {
            eprintln!("[正版检测] 步骤3：✗ Xbox Live 认证失败");
            "REFRESH_FAILED".to_string()
        })?;
    eprintln!("[正版检测] 步骤3：✓ Xbox Live 认证成功");

    let xsts_token = get_xsts_token(&client, &xbox_token.token)
        .await
        .map_err(|_| {
            eprintln!("[正版检测] 步骤3：✗ XSTS 认证失败");
            "REFRESH_FAILED".to_string()
        })?;
    eprintln!("[正版检测] 步骤3：✓ XSTS 认证成功");

    let mc_login = authenticate_with_minecraft(
        &client,
        &xsts_token.display_claims.xui[0].uhs,
        &xsts_token.token,
    )
    .await
    .map_err(|_| {
        eprintln!("[正版检测] 步骤3：✗ Minecraft 认证失败");
        "REFRESH_FAILED".to_string()
    })?;
    eprintln!("[正版检测] 步骤3：✓ Minecraft 认证成功，新 Minecraft access_token 长度={}", mc_login.access_token.len());

    // 4) 取得玩家资料（玩家名、皮肤等）
    let mc_profile = get_minecraft_profile(&client, &mc_login.access_token)
        .await
        .map_err(|_| {
            eprintln!("[正版检测] 步骤3：✗ 获取 Minecraft 个人资料失败");
            "REFRESH_FAILED".to_string()
        })?;
    eprintln!("[正版检测] 步骤3：✓ 获取个人资料成功，玩家名={}", mc_profile.name);

    // 5) 把新 token 存回数据库（再次单独开连接，同步块，不跨 await）
    {
        let connection = setup_database().map_err(|e| format!("打开账户数据库失败: {}", e))?;
        save_account_info(
            &connection,
            &mc_profile.name,
            &db_uuid,
            &refreshed.refresh_token,
            &mc_login.access_token,
        )
        .map_err(|e| format!("保存账号信息失败: {}", e))?;
        eprintln!("[正版检测] 步骤3：✓ 新凭据已保存到数据库");
    }

    // 6) 同步下载皮肤（如果需要）
    let _ = download_player_skin(&client, &db_uuid).await;
    eprintln!("[正版检测] 步骤3：✅ 静默刷新账号成功！一切正常");

    Ok(AccountInfo {
        name: mc_profile.name,
        uuid: db_uuid.clone(),
        auth_type: "microsoft".to_string(),
        access_token: mc_login.access_token,
        skin_url: Some(db_uuid),
    })
}

/// 检查数据库中是否存在某个 uuid 的微软账号（有 refresh_token 记录）
/// 用于启动时判断是否需要"试探正版账号是否正常登录"
#[tauri::command]
pub async fn ms_has_account_in_db(uuid: String) -> Result<bool, String> {
    eprintln!("[正版检测] 步骤1：检查数据库中是否存在账号 UUID={}", uuid);
    // 整个函数同步完成（没有 await），SQLite 非 Send 对象不会跨 await，Send 安全
    let connection = setup_database().map_err(|e| format!("打开账户数据库失败: {}", e))?;
    let query = format!(
        "SELECT COUNT(*) AS cnt FROM accounts WHERE uuid = '{}'",
        uuid.replace('\'', "''")
    );
    let mut stmt = connection.prepare(query).map_err(|e| e.to_string())?;
    if let State::Row = stmt.next().map_err(|e| e.to_string())? {
        let cnt: i64 = stmt.read::<i64, _>(0).unwrap_or(0);
        let found = cnt > 0;
        if found {
            eprintln!("[正版检测] 步骤1：✓ 数据库中存在该账号记录 (count={})", cnt);
        } else {
            eprintln!("[正版检测] 步骤1：✗ 数据库中不存在该账号，跳过后续检测");
        }
        Ok(found)
    } else {
        eprintln!("[正版检测] 步骤1：✗ 数据库查询失败，账号不存在");
        Ok(false)
    }
}
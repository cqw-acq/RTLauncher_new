use reqwest::Client;
use sqlite::State;
use serde::{Deserialize, Serialize};
use sqlite::Connection;
use std::time::Duration;
use tokio::time::sleep;
use std::fs;
use std::path::Path;
use base64::decode;
use tokio::time::Instant;

use super::AccountInfo;

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
    Token: String,
    DisplayClaims: DisplayClaims,
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
async fn get_device_code(client: &Client, client_id: &str) -> Result<DeviceCodeResponse, Box<dyn std::error::Error>> {
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
                Some(2148916233) => "该 Microsoft 账户未关联 Xbox 账户，请先前往 xbox.com 注册".to_string(),
                Some(2148916235) => "您所在地区不支持 Xbox Live，无法使用正版登录".to_string(),
                Some(2148916236) | Some(2148916237) => "需要在 Xbox 官网完成成人验证".to_string(),
                Some(2148916238) => "未成年账户需要家长在 Microsoft Family 中审批".to_string(),
                Some(code) => format!("XSTS 错误码: {}", code),
                None => err_resp.message.unwrap_or_else(|| format!("HTTP {}: {}", status, text)),
            }
        } else if text.is_empty() {
            format!("XSTS 服务器返回 HTTP {} 且响应体为空，可能是账户权限问题", status)
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

async fn check_mc_purchase(client: &Client, access_token: &str) -> Result<String, Box<dyn std::error::Error>> {
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
    let query = format!("SELECT uuid, refresh_token, access_token, time FROM accounts WHERE username = '{}'", username);
    let mut stmt = connection.prepare(query)?;

    if let State::Row = stmt.next()? {
        let uuid: String = stmt.read::<String, _>(0)?;
        let refresh_token: String = stmt.read::<String, _>(1)?;
        let access_token: String = stmt.read::<String, _>(2)?;
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
            let xsts_token_response = get_xsts_token(client, &xbox_token_response.Token).await?;
            let minecraft_login_response = authenticate_with_minecraft(
                client,
                &xbox_token_response.DisplayClaims.xui[0].uhs,
                &xsts_token_response.Token,
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

            let refreshed_token_response = refresh_access_token(client, client_id, &refresh_token).await?;

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
async fn download_player_skin(client: &Client, uuid: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 皮肤存到配置目录下的 skins 子目录
    let profile_dir = format!("{}/skins", super::config_dir());
    fs::create_dir_all(&profile_dir)?;

    // Get player profile to check if skin exists
    let profile_response = client
        .get(&format!("https://sessionserver.mojang.com/session/minecraft/profile/{}", uuid))
        .send()
        .await?;

    if !profile_response.status().is_success() {
        return Err("Failed to fetch player profile".into());
    }

    let profile_json: serde_json::Value = profile_response.json().await?;
    let properties = profile_json["properties"].as_array()
        .ok_or("No properties found in profile")?;

    // Find the textures property
    let textures_property = properties.iter()
        .find(|p| p["name"].as_str() == Some("textures"))
        .ok_or("No textures property found")?;

    // Decode the base64 textures value
    let textures_base64 = textures_property["value"].as_str()
        .ok_or("Textures value is not a string")?;
    let decoded = base64::decode(textures_base64)?;
    let textures_json: serde_json::Value = serde_json::from_slice(&decoded)?;

    // Get the skin URL
    let skin_url = textures_json["textures"]["SKIN"]["url"].as_str()
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
                let xbox_token_response = authenticate_with_xbox_live(client, &token.access_token).await?;

                // 4. 获取XSTS token
                let xsts_token_response = get_xsts_token(client, &xbox_token_response.Token).await?;

                // 5. Minecraft认证
                let minecraft_login_response = authenticate_with_minecraft(
                    client,
                    &xbox_token_response.DisplayClaims.xui[0].uhs,
                    &xsts_token_response.Token,
                )
                .await?;

                // 6. 检查是否拥有Minecraft
                let purchase_status = check_mc_purchase(client, &minecraft_login_response.access_token).await?;
                if purchase_status.contains("还没有购买") {
                    return Err(purchase_status.into());
                }

                // 7. 获取Minecraft个人资料
                let profile = get_minecraft_profile(client, &minecraft_login_response.access_token).await?;

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
    if msg.contains("connect") || msg.contains("connection") || msg.contains("timed out") || msg.contains("timeout") || msg.contains("dns") || msg.contains("resolve") {
        format!("网络连接失败，请检查您的网络后重试（{}）", msg)
    } else {
        msg
    }
}

/// 第一步：请求设备代码，前端展示 user_code 和 verification_uri
#[tauri::command]
pub async fn ms_request_device_code() -> Result<DeviceCodeInfo, String> {
    let client = build_http_client()?;
    let resp = get_device_code(&client, CLIENT_ID)
        .await
        .map_err(|e| friendly_net_err(e))?;
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
    let client = build_http_client()?;
    let start_time = Instant::now();
    let timeout = Duration::from_secs(300); // 5 分钟超时

    loop {
        if start_time.elapsed() >= timeout {
            return Err("登录超时，请重试".to_string());
        }

        sleep(Duration::from_secs(interval)).await;

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

        let token: TokenResponse = response.json().await
            .map_err(|e| format!("解析 Token 失败: {}", e))?;

        // Xbox Live 认证
        let xbox = authenticate_with_xbox_live(&client, &token.access_token)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("timed out") || msg.contains("timeout") || msg.contains("dns") || msg.contains("resolve") {
                    format!("网络连接失败，请检查您的网络后重试（{}）", msg)
                } else {
                    format!("Xbox Live 认证失败: {}", msg)
                }
            })?;;

        // XSTS Token
        let xsts = get_xsts_token(&client, &xbox.Token)
            .await
            .map_err(|e| format!("XSTS 认证失败: {}", e))?;

        // Minecraft 认证
        let uhs = xbox.DisplayClaims.xui.first()
            .map(|x| x.uhs.clone())
            .ok_or_else(|| "Xbox Live 认证返回的 xui 为空".to_string())?;
        let mc_login = authenticate_with_minecraft(
            &client,
            &uhs,
            &xsts.Token,
        )
        .await
        .map_err(|e| format!("Minecraft 认证失败: {}", e))?;

        // 检查是否拥有 Minecraft
        let purchase = check_mc_purchase(&client, &mc_login.access_token)
            .await
            .map_err(|e| format!("检查购买状态失败: {}", e))?;
        if purchase.contains("还没有购买") {
            return Err(purchase);
        }

        // 获取 Minecraft 个人资料
        let profile = get_minecraft_profile(&client, &mc_login.access_token)
            .await
            .map_err(|e| format!("获取 Minecraft 资料失败: {}", e))?;

        // 构造返回值（先于数据库/皮肤操作，确保即使后续失败也能返回）
        let account_info = AccountInfo {
            name: profile.name.clone(),
            uuid: profile.id.clone(),
            auth_type: "microsoft".to_string(),
            access_token: mc_login.access_token.clone(),
            skin_url: None,
        };

        // 保存到数据库（非致命，通过 spawn_blocking 隔离 sqlite 线程安全问题）
        let db_name = profile.name.clone();
        let db_id = profile.id.clone();
        let db_refresh = token.refresh_token.clone();
        let db_access = mc_login.access_token.clone();
        let db_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let connection = setup_database().map_err(|e| e.to_string())?;
            save_account_info(
                &connection,
                &db_name,
                &db_id,
                &db_refresh,
                &db_access,
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }).await;
        match db_result {
            Ok(Ok(())) => {},
            Ok(Err(e)) => eprintln!("[MS登录] 数据库保存失败(非致命): {}", e),
            Err(e) => eprintln!("[MS登录] 数据库任务崩溃(非致命): {}", e),
        }

        // 下载皮肤（非致命）
        match download_player_skin(&client, &profile.id).await {
            Ok(()) => {},
            Err(e) => eprintln!("[MS登录] 皮肤下载失败(非致命): {}", e),
        }

        return Ok(account_info);
    }
}
//! LittleSkin 账户管理
//!
//! 提供两种登录方式：
//!   1. OAuth 方式（原有的，通过浏览器跳转授权）
//!   2. Yggdrasil 用户名/密码登录（PCL2 风格，无需浏览器）
//!
//! 数据库中每个 UUID 只存一条记录，支持多账户。

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlite::Connection;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use url::Url;

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct YggdrasilAuthenticateRequest {
    agent: YggdrasilAgent,
    username: String,
    password: String,
    client_token: Option<String>,
    request_user: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct YggdrasilAgent {
    name: String,
    version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct YggdrasilAvailableProfile {
    id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct YggdrasilSelectedProfile {
    id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct YggdrasilUser {
    id: String,
    username: Option<String>,
    properties: Option<Vec<HashMap<String, String>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct YggdrasilAuthenticateResponse {
    accessToken: String,
    clientToken: String,
    #[serde(rename = "availableProfiles")]
    available_profiles: Vec<YggdrasilAvailableProfile>,
    #[serde(rename = "selectedProfile")]
    selected_profile: Option<YggdrasilSelectedProfile>,
    user: Option<YggdrasilUser>,
}

/// 返回给前端的结构化账户信息
#[derive(Debug, Serialize, Deserialize)]
pub struct LittleSkinAccountResult {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub skin_url: Option<String>,
}

// ---------------------------------------------------------------------------
// 数据库工具
// ---------------------------------------------------------------------------

/// 确保表存在（幂等，不 DROP）
fn ensure_tables(conn: &Connection) -> Result<(), String> {
    let queries = [
        "CREATE TABLE IF NOT EXISTS littleskin_user (
            uuid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            access_token TEXT NOT NULL,
            refresh_token TEXT,
            tid_skin INTEGER,
            created_at INTEGER DEFAULT (strftime('%s','now'))
        );",
        "CREATE TABLE IF NOT EXISTS littleskin_token (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            access_token TEXT NOT NULL,
            refresh_token TEXT
        );",
    ];
    for q in queries {
        conn.execute(q).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 保存/更新 LittleSkin 玩家信息（以 uuid 为主键，实现多账户）
fn upsert_player(
    conn: &Connection,
    uuid: &str,
    name: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    tid_skin: Option<i64>,
) -> Result<(), String> {
    let query = "INSERT INTO littleskin_user (uuid, name, access_token, refresh_token, tid_skin)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(uuid) DO UPDATE SET
                    name=excluded.name,
                    access_token=excluded.access_token,
                    refresh_token=excluded.refresh_token,
                    tid_skin=excluded.tid_skin;";
    let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;
    stmt.bind((1, uuid)).map_err(|e| e.to_string())?;
    stmt.bind((2, name)).map_err(|e| e.to_string())?;
    stmt.bind((3, access_token)).map_err(|e| e.to_string())?;
    stmt.bind((4, refresh_token.unwrap_or("")))
        .map_err(|e| e.to_string())?;
    stmt.bind((5, tid_skin.unwrap_or(0) as i64))
        .map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// OAuth 登录方式（保留，与原实现一致）
// ---------------------------------------------------------------------------

fn check_network_connection() -> Result<(), String> {
    let test_hosts = vec![
        ("8.8.8.8:53", "Google DNS"),
        ("1.1.1.1:53", "Cloudflare DNS"),
        ("114.114.114.114:53", "114 DNS"),
    ];

    for (host, _name) in test_hosts {
        if let Ok(mut addrs) = host.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                if TcpStream::connect_timeout(&addr, Duration::from_secs(3)).is_ok() {
                    return Ok(());
                }
            }
        }
    }

    Err("无法连接到网络，请检查网络连接后重试".to_string())
}

struct LittleSkinClient {
    client: Client,
    redirect_uri: String,
    client_id: String,
    client_secret: String,
    code: Arc<Mutex<Option<String>>>,
}

impl LittleSkinClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            redirect_uri: "http://localhost:40323".to_string(),
            client_id: "1340".to_string(),
            client_secret: "vF9bybNWzp3AzSrICE6pZMrMzZgEKJtdf8HTz9Ep".to_string(),
            code: Arc::new(Mutex::new(None)),
        }
    }

    pub fn authenticate_and_return(&mut self) -> Result<super::AccountInfo, String> {
        check_network_connection()?;

        let authorize_url = format!(
            "https://littleskin.cn/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=Player.ReadWrite",
            self.client_id, self.redirect_uri
        );

        let _ = webbrowser::open(&authorize_url);

        let bind_address = self.redirect_uri.replace("http://", "");
        let listener = TcpListener::bind(&bind_address).map_err(|e| format!("无法绑定地址: {}", e))?;

        let code_clone = Arc::clone(&self.code);
        let start_time = Instant::now();

        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let mut buffer = [0; 1024];
                    let _ = stream.read(&mut buffer);
                    if let Ok(request) = String::from_utf8(buffer.to_vec()) {
                        let lines: Vec<&str> = request.split('\r').collect();
                        let request_line = lines[0].trim();
                        if let Some(url) = request_line.split_whitespace().nth(1) {
                            if let Ok(parsed_url) =
                                Url::parse(&format!("http://localhost{}", url))
                            {
                                let query_pairs: HashMap<_, _> =
                                    parsed_url.query_pairs().into_owned().collect();
                                if let Some(code) = query_pairs.get("code") {
                                    *code_clone.lock().unwrap() = Some(code.clone());
                                }
                            }
                        }
                        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>授权成功！请关闭此页面</h1></body></html>";
                        let _ = stream.write(response.as_bytes());
                        let _ = stream.flush();
                    }
                    break;
                }
            }
        });

        loop {
            if let Some(code) = self.code.lock().unwrap().take() {
                let _ = handle.join();
                let token_response = self.request_token(&code)?;
                let player_info = self.get_player_info(&token_response.access_token);
                let player_data: Value =
                    serde_json::from_str(&player_info).unwrap_or(Value::Null);

                let conn = Connection::open(super::db_path()).map_err(|e| e.to_string())?;
                ensure_tables(&conn)?;

                if let Some(players) = player_data.as_array() {
                    if let Some(first) = players.first() {
                        let name = first
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let uuid = first
                            .get("uuid")
                            .and_then(|u| u.as_str())
                            .unwrap_or("")
                            .to_string();
                        let tid_skin = first.get("tid_skin").and_then(|t| t.as_i64());

                        // 下载真实皮肤 PNG 到本地 (供 3D 展示)
                        let _ = crate::auth::official::download_skin_blocking(
                            &uuid,
                            "https://littleskin.cn/api/yggdrasil",
                        );

                        upsert_player(
                            &conn,
                            &uuid,
                            &name,
                            &token_response.access_token,
                            Some(&token_response.refresh_token),
                            tid_skin,
                        )?;

                        return Ok(super::AccountInfo {
                            name,
                            uuid: uuid.clone(),
                            auth_type: "littleskin".to_string(),
                            access_token: token_response.access_token,
                            skin_url: Some(uuid),
                        });
                    }
                }
                return Err("未找到有效的玩家信息".to_string());
            }
            thread::sleep(Duration::from_secs(1));
            if start_time.elapsed() >= Duration::from_secs(360) {
                let _ = handle.join();
                return Err("登录超时，请重新登录".to_string());
            }
        }
    }

    fn request_token(&self, code: &str) -> Result<TokenResponse, String> {
        let mut params = HashMap::new();
        params.insert("grant_type".to_string(), "authorization_code".to_string());
        params.insert("client_id".to_string(), self.client_id.clone());
        params.insert("client_secret".to_string(), self.client_secret.clone());
        params.insert("redirect_uri".to_string(), self.redirect_uri.clone());
        params.insert("code".to_string(), code.to_string());

        let response = self
            .client
            .post("https://littleskin.cn/oauth/token")
            .form(&params)
            .send()
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            let response_text = response.text().map_err(|e| e.to_string())?;
            let token_response: TokenResponse =
                serde_json::from_str(&response_text).map_err(|e| e.to_string())?;
            Ok(token_response)
        } else {
            let error_text = response.text().map_err(|e| e.to_string())?;
            Err(error_text)
        }
    }

    fn get_player_info(&self, access_token: &str) -> String {
        let response = self
            .client
            .get("https://littleskin.cn/api/players")
            .header("Authorization", format!("Bearer {}", access_token))
            .send();

        match response {
            Ok(resp) => match resp.text() {
                Ok(text) => text,
                Err(e) => format!("无法读取玩家信息响应: {}", e),
            },
            Err(e) => format!("无法发送玩家信息请求: {}", e),
        }
    }
}

// ---------------------------------------------------------------------------
// Yggdrasil 用户名/密码登录方式（PCL2 风格，无需浏览器）
// ---------------------------------------------------------------------------

/// Yggdrasil 认证端点 (LittleSkin)
const YGGDRASIL_AUTH_BASE: &str = "https://littleskin.cn/api/yggdrasil";

/// 使用 LittleSkin 用户名/密码登录
///
/// # Arguments
/// * `username` - LittleSkin 账号（邮箱或用户名）
/// * `password` - LittleSkin 账号密码
///
/// # Returns
/// 玩家信息列表（一个 LittleSkin 账号可能关联多个玩家角色）
pub fn authenticate_with_credentials(
    username: &str,
    password: &str,
) -> Result<Vec<LittleSkinAccountResult>, String> {
    check_network_connection().map_err(|_| "网络连接异常，请检查网络".to_string())?;

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("RTLauncher/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    // 生成随机 client_token
    let client_token = format!("rtl-{}", uuid_v4());

    // 构造 Yggdrasil authenticate 请求
    let body = json!({
        "agent": { "name": "Minecraft", "version": 1 },
        "username": username,
        "password": password,
        "clientToken": client_token,
        "requestUser": true,
    });

    let auth_url = format!("{}/authserver/authenticate", YGGDRASIL_AUTH_BASE);
    let response = client
        .post(&auth_url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .map_err(|e| format!("登录请求失败: {}", e))?;

    let status = response.status();
    let response_text = response.text().map_err(|e| e.to_string())?;

    if !status.is_success() {
        // 尝试解析错误信息
        let err: Result<Value, _> = serde_json::from_str(&response_text);
        let msg = match err {
            Ok(v) => v
                .get("errorMessage")
                .and_then(|m| m.as_str())
                .or_else(|| v.get("error").and_then(|e| e.as_str()))
                .unwrap_or("账号或密码错误")
                .to_string(),
            Err(_) => format!("认证失败 (HTTP {})", status),
        };
        return Err(msg);
    }

    let parsed: YggdrasilAuthenticateResponse =
        serde_json::from_str(&response_text).map_err(|e| format!("响应解析失败: {}", e))?;

    // 构建返回结果：每个 available profile 对应一条账户
    let mut results: Vec<LittleSkinAccountResult> = Vec::new();

    if parsed.available_profiles.is_empty() {
        // 如果没有可用角色，使用 selectedProfile 或 user 信息
        if let Some(sp) = &parsed.selected_profile {
            let _ = crate::auth::official::download_skin_blocking(
                &format_uuid(&sp.id),
                YGGDRASIL_AUTH_BASE,
            );
            results.push(LittleSkinAccountResult {
                name: sp.name.clone(),
                uuid: format_uuid(&sp.id),
                access_token: parsed.accessToken.clone(),
                skin_url: Some(format_uuid(&sp.id)),
            });
        } else if let Some(user) = &parsed.user {
            results.push(LittleSkinAccountResult {
                name: user.username.clone().unwrap_or_else(|| "Player".to_string()),
                uuid: format_uuid(&user.id),
                access_token: parsed.accessToken.clone(),
                skin_url: Some(format_uuid(&user.id)),
            });
        } else {
            return Err("账户未创建玩家角色，请先在 LittleSkin 创建角色".to_string());
        }
    } else {
        for profile in &parsed.available_profiles {
            let _ = crate::auth::official::download_skin_blocking(
                &format_uuid(&profile.id),
                YGGDRASIL_AUTH_BASE,
            );
            results.push(LittleSkinAccountResult {
                name: profile.name.clone(),
                uuid: format_uuid(&profile.id),
                access_token: parsed.accessToken.clone(),
                skin_url: Some(format_uuid(&profile.id)),
            });
        }
    }

    if results.is_empty() {
        return Err("未能获取玩家信息".to_string());
    }

    // 保存到数据库（多账户支持）
    let conn = Connection::open(super::db_path()).map_err(|e| e.to_string())?;
    ensure_tables(&conn)?;
    for r in &results {
        upsert_player(
            &conn,
            &r.uuid,
            &r.name,
            &r.access_token,
            None,
            None,
        )?;
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 生成 v4 UUID
fn uuid_v4() -> String {
    // 简单的随机 UUID v4 生成（纯 Rust，无需额外 crate）
    let mut bytes = [0u8; 16];
    // 用时间 + 伪随机填充
    let now = Instant::now();
    let ns = now.elapsed().as_nanos() as u64;
    bytes[0..8].copy_from_slice(&ns.to_le_bytes());
    bytes[8..16].copy_from_slice(&(ns.wrapping_mul(0x9E3779B97F4A7C15)).to_le_bytes());
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant RFC4122

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11],
        bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

/// 将无连字符的 UUID 格式化为标准 8-4-4-4-12 形式
fn format_uuid(id: &str) -> String {
    let clean: String = id.chars().filter(|c| c.is_alphanumeric()).collect();
    if clean.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &clean[0..8],
            &clean[8..12],
            &clean[12..16],
            &clean[16..20],
            &clean[20..32],
        )
    } else {
        id.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tauri 命令
// ---------------------------------------------------------------------------

/// 原有的 OAuth 登录命令（保留以兼容旧接口）
#[tauri::command]
pub fn useMethod() -> Result<super::AccountInfo, String> {
    let mut client = LittleSkinClient::new();
    client.authenticate_and_return()
}

/// 新命令：使用 LittleSkin 账号密码直接登录（返回玩家角色列表）
#[tauri::command]
pub fn use_method_with_credentials(
    username: String,
    password: String,
) -> Result<Vec<LittleSkinAccountResult>, String> {
    if username.trim().is_empty() {
        return Err("用户名不能为空".to_string());
    }
    if password.trim().is_empty() {
        return Err("密码不能为空".to_string());
    }
    authenticate_with_credentials(username.trim(), password.trim())
}
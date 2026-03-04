    use reqwest::blocking::Client;
    use sqlite::{Connection, State};
    use std::collections::HashMap;
    use std::net::{TcpListener, TcpStream};
    use std::io::{Read, Write};
    use std::thread;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use url::Url;
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use webbrowser;

    #[derive(Debug, Serialize, Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
        expires_in: u64,
        token_type: String,
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

        pub fn authenticate(&mut self) -> String {
            let authorize_url = format!(
                "https://littleskin.cn/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=Player.ReadWrite",
                self.client_id, self.redirect_uri
            );

            if let Err(e) = webbrowser::open(&authorize_url) {
                return format!("无法打开浏览器: {}", e);
            }

            println!("正在打开浏览器进行授权...");
            println!("等待授权回调中...");

            let bind_address = self.redirect_uri.replace("http://", "");
            let listener = TcpListener::bind(&bind_address).expect("无法绑定到地址");
            println!("正在监听重定向 URL: {}", self.redirect_uri);

            let code_clone = Arc::clone(&self.code);
            let start_time = Instant::now();

            let handle = thread::spawn(move || {
                for stream in listener.incoming() {
                    if let Ok(mut stream) = stream {
                        let mut buffer = [0; 1024];
                        stream.read(&mut buffer).unwrap();

                        if let Ok(request) = String::from_utf8(buffer.to_vec()) {
                            let lines: Vec<&str> = request.split('\r').collect();
                            let request_line = lines[0].trim();
                            if let Some(url) = request_line.split_whitespace().nth(1) {
                                if let Ok(parsed_url) = Url::parse(&format!("http://localhost{}", url)) {
                                    if let Some(query) = parsed_url.query() {
                                        let query_pairs: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();
                                        if let Some(code) = query_pairs.get("code") {
                                            *code_clone.lock().unwrap() = Some(code.clone());
                                            break;
                                        }
                                    }
                                }
                            }

                            let response = "HTTP/1.1 200 OK\r
    Content-Type: text/html\r
    \r
    <html><body><h1>授权成功！请关闭此页面</h1></body></html>";
                            stream.write(response.as_bytes()).unwrap();
                            stream.flush().unwrap();
                        }
                    }
                }
            });

            for _ in 0..360 {
                if let Some(code) = self.code.lock().unwrap().take() {
                    handle.join().unwrap();
                    match self.request_token(&code) {
                        Ok(token_response) => {
                            let player_info = self.get_player_info(&token_response.access_token);
                            
                            let player_data: Value = serde_json::from_str(&player_info)
                                .unwrap_or(Value::Null);

                            if let Err(e) = self.handle_database(&token_response, &player_data) {
                                return format!("数据库操作失败: {}", e);
                            }

                            let display_info = self.extract_display_info(&player_data);
                            
                            return format!("登录成功！
    {}", display_info);
                        }
                        Err(error) => {
                            return format!("登录失败: {:?}", error);
                        }
                    }
                }
                thread::sleep(Duration::from_secs(1));
                if start_time.elapsed() >= Duration::from_secs(360) {
                    handle.join().unwrap();
                    return "登录超时，请重新登录".to_string();
                }
            }

            handle.join().unwrap();
            "未获取到授权码".to_string()
        }

        fn handle_database(&self, token_response: &TokenResponse, player_data: &Value) -> Result<(), String> {
        // Open or create database
        let connection = match Connection::open("LaunchAccount.db") {
            Ok(conn) => conn,
            Err(e) => return Err(format!("无法打开数据库: {}", e)),
        };

        // 创建表（如果不存在）
        let queries = vec![
            
            "DROP TABLE IF EXISTS littleskin;
            DROP TABLE IF EXISTS littleskinuser;
            CREATE TABLE littleskin (
            refresh_token TEXT NOT NULL,
            access_token TEXT NOT NULL
            );

            CREATE TABLE littleskinuser (
            name TEXT NOT NULL,
            tid_skin INTEGER NOT NULL,
            uuid TEXT PRIMARY KEY
            );"
        ];

        

        for query in queries {
            if let Err(e) = connection.execute(query) {
                return Err(format!("无法创建表: {} - {}", query, e));
            }
        }

        // 删除旧的 access_token 和 refresh_token
        connection.execute("DELETE FROM littleskin").map_err(|e| e.to_string())?;

        // 插入新的 access_token 和 refresh_token
        let insert_token_query = "INSERT INTO littleskin (refresh_token, access_token) VALUES (?, ?)";
        let mut statement = connection.prepare(insert_token_query).map_err(|e| e.to_string())?;
        statement.bind((1, &token_response.refresh_token as &str)).map_err(|e| e.to_string())?;
        statement.bind((2, &token_response.access_token as &str)).map_err(|e| e.to_string())?;
        statement.next().map_err(|e| e.to_string())?;

        // 删除旧的玩家信息
        connection.execute("DELETE FROM littleskinuser").map_err(|e| e.to_string())?;

        // 插入新的玩家信息
        if let Some(players) = player_data.as_array() {
            for player in players {
                if let (Some(uuid), Some(name), Some(tid_skin)) = (
                    player.get("uuid").and_then(|u| u.as_str()),
                    player.get("name").and_then(|n| n.as_str()),
                    player.get("tid_skin").and_then(|t| t.as_i64()),
                ) {
                    let query = "
                        INSERT INTO littleskinuser 
                        (uuid, name, tid_skin) 
                        VALUES (?, ?, ?)";
                    
                    let mut statement = match connection.prepare(query) {
                        Ok(stmt) => stmt,
                        Err(e) => return Err(format!("无法准备插入语句: {}", e)),
                    };
                    
                    if let Err(e) = statement.bind((1, uuid)) {
                        return Err(format!("无法绑定uuid: {}", e));
                    }
                    if let Err(e) = statement.bind((2, name)) {
                        return Err(format!("无法绑定name: {}", e));
                    }
                    if let Err(e) = statement.bind((3, tid_skin)) {
                        return Err(format!("无法绑定tid_skin: {}", e));
                    }
                    
                    if let Err(e) = statement.next() {
                        return Err(format!("无法执行插入: {}", e));
                    }
                }
            }
        }

        Ok(())
    }
        fn extract_display_info(&self, player_data: &Value) -> String {
            let mut result = String::new();
            
            if let Some(players) = player_data.as_array() {
                if let Some(first_player) = players.first() {
                    if let (Some(name), Some(tid_skin), Some(uuid)) = (
                        first_player.get("name").and_then(|n| n.as_str()),
                        first_player.get("tid_skin").and_then(|t| t.as_i64()),
                        first_player.get("uuid").and_then(|u| u.as_str()),
                    ) {
                        result.push_str(&format!("角色名: {}
    皮肤ID: {}
    UUID: {}", name, tid_skin, uuid));
                    }
                }
            }
            
            if result.is_empty() {
                "未找到有效的玩家信息".to_string()
            } else {
                result
            }
        }

        fn request_token(&self, code: &str) -> Result<TokenResponse, String> {
            let mut params = HashMap::new();
            params.insert("grant_type".to_string(), "authorization_code".to_string());
            params.insert("client_id".to_string(), self.client_id.clone());
            params.insert("client_secret".to_string(), self.client_secret.clone());
            params.insert("redirect_uri".to_string(), self.redirect_uri.clone());
            params.insert("code".to_string(), code.to_string());

            let response = self.client
                .post("https://littleskin.cn/oauth/token")
                .form(&params)
                .send()
                .map_err(|e| e.to_string())?;

            if response.status().is_success() {
                let response_text = response.text().map_err(|e| e.to_string())?;
                println!("成功获取令牌的响应: {}", response_text);

                let token_response: TokenResponse = serde_json::from_str(&response_text)
                    .map_err(|e| e.to_string())?;

                Ok(token_response)
            } else {
                let error_text = response.text().map_err(|e| e.to_string())?;
                println!("登录失败的响应: {}", error_text);
                Err(error_text)
            }
        }

        fn get_player_info(&self, access_token: &str) -> String {
            let response = self.client
                .get("https://littleskin.cn/api/players")
                .header("Authorization", format!("Bearer {}", access_token))
                .send();

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let response_text = resp.text().map_err(|e| e.to_string());

                    match response_text {
                        Ok(text) => {
                            if status.is_success() {
                                println!("成功获取玩家信息");
                                text
                            } else {
                                println!("获取玩家信息失败");
                                text
                            }
                        }
                        Err(e) => {
                            println!("无法读取玩家信息响应: {}", e);
                            format!("无法读取玩家信息响应: {}", e)
                        }
                    }
                }
                Err(e) => {
                    println!("无法发送玩家信息请求: {}", e);
                    format!("无法发送玩家信息请求: {}", e)
                }
            }
        }
    }
    #[tauri::command]
    pub fn useMethod() -> Result<(), Box<dyn std::error::Error>>{    // 创建 LittleSkinClient 实例
        let mut client = LittleSkinClient::new();

        // 调用 authenticate 方法进行登录
        let result = client.authenticate();

        // 打印登录结果
        println!("{}", result);
        Ok(())
    }


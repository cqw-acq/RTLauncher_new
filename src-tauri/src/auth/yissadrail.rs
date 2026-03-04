/*
RTLauncher, a third-party Minecraft launcher built with the newest
technology and provides innovative funtionalities
Copyright (C) 2025 lutouna

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use log::{info, error, warn};
use sha2::{Sha256, Digest};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

const FOLDER_PATH: &str = "./.minecraft/versions";

pub fn downloadInjecter() {
    // 初始化
    let URL_BMCL = "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json";
    let URL_YUSHI = "https://authlib-injector.yushi.moe/artifact/latest.json";
    let httpClient = reqwest::blocking::Client::new();

    // 尝试bmcl源
    let mut jsonResponse = httpClient.get(URL_BMCL).send();
    let mut jsonData;
    
    // 尝试yushi源
    if jsonResponse.is_err() {
        jsonResponse = httpClient.get(URL_YUSHI).send();
        if jsonResponse.is_err() {
            error!("两个链接都连接失败了");
            return;
        }
    }
    
    // 解析JSON
    let jsonText = jsonResponse.unwrap().text();
    if jsonText.is_err() {
        error!("获取JSON失败");
        return;
    }
    
    jsonData = serde_json::from_str(&jsonText.unwrap());
    if jsonData.is_err() {
        error!("JSON格式错误");
        return;
    }
    
    let jsonData: serde_json::Value = jsonData.unwrap();
    
    // 获取下载地址
    let downloadUrl = jsonData.get("download_url");
    if downloadUrl.is_none() {
        error!("JSON中没有下载地址");
        return;
    }
    let downloadUrl = downloadUrl.unwrap().as_str();
    if downloadUrl.is_none() {
        error!("下载地址格式错误");
        return;
    }
    let downloadUrl = downloadUrl.unwrap();
    
    // 获取文件名
    let urlParts: Vec<&str> = downloadUrl.split('/').collect();
    let fileName = urlParts.last().unwrap_or(&"authlib-injector.jar");
    
    // 构造文件路径
    let filePath = format!("{}/{}", FOLDER_PATH, fileName);
    
    // 检查文件是否已存在
    if fs::metadata(&filePath).is_ok() {
        let fileContent = fs::read(&filePath);
        if fileContent.is_err() {
            error!("读取现有文件失败");
            return;
        }
        let fileContent = fileContent.unwrap();
        let fileSha256 = hex::encode(Sha256::digest(&fileContent));
        
        // 获取校验和
        let checksumValue = jsonData.get("checksums")
            .and_then(|c| c.get("sha256"))
            .and_then(|s| s.as_str());
        if checksumValue.is_none() {
            error!("无法获取校验值");
            return;
        }
        let checksumValue = checksumValue.unwrap();
        
        if fileSha256 == checksumValue {
            info!("文件已存在且校验成功");
            return;
        }
        let downloadResponse = httpClient.get(downloadUrl).send();
        if downloadResponse.is_err() {
            error!("下载文件失败");
            return;
        }
        info!("文件已更新");
        return;
    }
    
    // 下载文件
    let downloadResponse = httpClient.get(downloadUrl).send();
    if downloadResponse.is_err() {
        error!("下载文件失败");
        return;
    }
    
    let fileContent = downloadResponse.unwrap().bytes();
    if fileContent.is_err() {
        error!("读取下载内容失败");
        return;
    }

    // 创建目录
    if let Err(err) = fs::create_dir_all(FOLDER_PATH) {
        error!("创建目录失败: {}", err);
        return;
    }

    // 保存文件
    let writeResult = fs::write(&filePath, fileContent.unwrap());
    if writeResult.is_err() {
        error!("保存文件失败: {}", writeResult.err().unwrap());
        return;
    }
    
    // 验证文件
    let checksumValue = jsonData.get("checksums")
        .and_then(|c| c.get("sha256"))
        .and_then(|s| s.as_str());
    if checksumValue.is_none() {
        error!("无法获取校验值");
        return;
    }
    let checksumValue = checksumValue.unwrap();
    
    // 计算文件的SHA256
    let mut fileHandle = fs::File::open(&filePath).unwrap();
    let mut fileBytes = Vec::new();
    fileHandle.read_to_end(&mut fileBytes).unwrap();
    
    let mut hasher = Sha256::new();
    hasher.update(&fileBytes);
    let fileSha256 = hex::encode(hasher.finalize());
    
    // 比较校验值
    if fileSha256 == checksumValue {
        info!("文件下载成功，校验成功");
    } else {
        error!("文件下载成功，校验失败");
    }
}

pub fn thirdPartyLogin(url: String) -> String {
    // 发送get请求
    let response = reqwest::blocking::get(url);
    if response.is_err() {
        error!("无法发送get请求: {}", response.err().unwrap());
        return String::new();
    }

    // 获取响应体
    let body = response.unwrap().text();
    if body.is_err() {
        error!("无法获取响应体: {}", body.err().unwrap());
        return String::new();
    }
    let bodyText = body.unwrap();

    // 查找返回的json中是否有signaturePublickey
    let jsonResult = serde_json::from_str::<serde_json::Value>(&bodyText);
    if jsonResult.is_err() {
        error!("JSON解析失败: {}，收到的内容: {}", jsonResult.err().unwrap(), bodyText);
        return String::new();
    }
    
    let json = jsonResult.unwrap();
    let signaturePublicKey = json.get("signaturePublickey");
    if signaturePublicKey.is_none() {
        error!("返回的json中没有signaturePublickey");
        return String::new();
    }

    // 返回base64编码的整个响应的json
    let base64Json = base64::encode(&bodyText);
    info!("{}", base64Json);
    base64Json
}

// 返回一个字符串和两个字符串数组
pub fn getAccountList(url: String, user: String, pwd: String) -> (String, Vec<String>, Vec<String>) {
    // 初始化
    let fullUrl = format!("{}/{}", url, "authserver/authenticate");
    let client = reqwest::blocking::Client::new();

    // 发送post请求
    let requestBody = format!(
        r#"{{"username":"{}","password":"{}","clientToken":"","requestUser":true,"agent":{{"name":"Minecraft","version":1}}}}"#,
        user, pwd
    );
    
    let response = client.post(fullUrl)
        .header("Content-Type", "application/json")
        .body(requestBody)
        .send();
        
    if response.is_err() {
        error!("发送POST请求失败: {}", response.err().unwrap());
        return (String::new(), Vec::new(), Vec::new());
    }

    // 获取响应体
    let body = response.unwrap().text();
    if body.is_err() {
        error!("获取响应体失败: {}", body.err().unwrap());
        return (String::new(), Vec::new(), Vec::new());
    }
    let bodyText = body.unwrap();

    // 解析JSON
    let jsonResult = serde_json::from_str::<serde_json::Value>(&bodyText);
    if jsonResult.is_err() {
        error!("JSON解析失败: {}", jsonResult.err().unwrap());
        return (String::new(), Vec::new(), Vec::new());
    }
    let json = jsonResult.unwrap();
    println!("{}",json);
    // 取accessToken
    let accessToken = json.get("accessToken");
    if accessToken.is_none() {
        error!("JSON中没有accessToken");
        return (String::new(), Vec::new(), Vec::new());
    }
    let accessToken = accessToken.unwrap().to_string();

    // 遍历json中的availableProfiles列表
    let availableProfiles = json.get("availableProfiles");
    if availableProfiles.is_none() {
        error!("JSON中没有availableProfiles");
        return (String::new(), Vec::new(), Vec::new());
    }

    let mut idList: Vec<String> = Vec::new();
    let mut nameList: Vec<String> = Vec::new();
    
    for profile in availableProfiles.unwrap().as_array().unwrap() {
        if let (Some(id), Some(name)) = (profile.get("id"), profile.get("name")) {
            idList.push(id.to_string());
            nameList.push(name.to_string());
        } else {
            warn!("json中缺少id或name字段");
        }
    }

    info!("accessToken: {}", accessToken);
    info!("idList: {:?}", idList);
    info!("nameList: {:?}", nameList);
    
    (accessToken, idList, nameList)
}

pub fn getPlayerSkin(url: String, uuid: String) -> String {
    info!("开始获取玩家皮肤...");
    
    // 去掉可能存在的引号
    let uuid = uuid.trim_matches('"');
    
    // 初始化
    let fullUrl = format!("{}/sessionserver/session/minecraft/profile/{}", url, uuid);
    info!("请求URL: {}", fullUrl);
    let httpClient = reqwest::blocking::Client::new();

    // 发送get请求
    let response = httpClient.get(fullUrl).send();
    if response.is_err() {
        error!("发送get请求失败: {}", response.err().unwrap());
        return String::new();
    }
    
    // 获取响应体
    let body = response.unwrap().text();
    if body.is_err() {
        error!("获取响应体失败: {}", body.err().unwrap());
        return String::new();
    }
    let bodyText = body.unwrap();

    // 解析JSON
    let jsonResult = serde_json::from_str::<serde_json::Value>(&bodyText);
    if jsonResult.is_err() {
        error!("JSON解析失败: {}", jsonResult.err().unwrap());
        return String::new();
    }
    let json = jsonResult.unwrap();

    // 获取 properties 中 value 的值
    let properties = json.get("properties");
    if properties.is_none() {
        error!("JSON中没有properties");
        return String::new();
    }
    let properties = properties.unwrap().as_array().unwrap();
    let value = properties.get(0).unwrap().get("value").unwrap().as_str().unwrap();
    info!("获取到的base64编码值: {}", value);

    // base64解码
    let decoded = BASE64.decode(value);
    if decoded.is_err() {
        error!("base64解码失败: {}", decoded.err().unwrap());
        return String::new();
    }

    // 解析解码后的json
    let jsonResult = serde_json::from_slice(&decoded.unwrap());
    if jsonResult.is_err() {
        error!("JSON解析失败: {}", jsonResult.err().unwrap());
        return String::new();
    }
    let json: serde_json::Value = jsonResult.unwrap();

    // 获取textures中的SKIN中的url
    let textures = json.get("textures");
    if textures.is_none() {
        error!("JSON中没有textures");
        return String::new();
    }
    let textures = textures.unwrap().as_object().unwrap();
    let skin = textures.get("SKIN").unwrap()
        .get("url").unwrap()
        .as_str().unwrap();
    
    info!("获取到的皮肤URL: {}", skin);
    
    // 下载皮肤
    let response = httpClient.get(skin).send();
    if response.is_err() {
        error!("下载皮肤失败: {}", response.err().unwrap());
        return String::new();
    }

    // 创建 skins 目录
    if let Err(err) = fs::create_dir_all("./skins") {
        error!("创建皮肤目录失败: {}", err);
        return String::new();
    }

    // 保存皮肤
    let skinPath = format!("./skins/{}.png", uuid);
    let mut file = match File::create(&skinPath) {
        Ok(file) => file,
        Err(err) => {
            error!("创建文件失败: {}", err);
            return String::new();
        }
    };
    file.write_all(&response.unwrap().bytes().unwrap()).unwrap();
    file.flush().unwrap();

    // 返回皮肤路径
    info!("皮肤路径: {}", skinPath);
    skinPath
}

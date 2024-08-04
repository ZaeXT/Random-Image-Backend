use std::collections::HashMap;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_web::cookie::Key;
use actix_session::{Session, SessionMiddleware};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use reqwest::Client;
use uuid::Uuid;
use env_logger::Env;

#[derive(Serialize, Deserialize)]
struct ImageResponse {
    #[serde(rename = "code")]
    resp_code: i32,
    #[serde(rename = "url")]
    image_url: String,
    #[serde(rename = "width")]
    image_width: i32,
    #[serde(rename = "height")]
    image_height: i32,
}

struct AppState {
    image_map: Mutex<HashMap<i32, String>>,
}

async fn handle_request(
    session: Session,
    path: web::Path<(String,)>,
    query: web::Query<HashMap<String, String>>,
    data: web::Data<AppState>,
    client: web::Data<Client>,
) -> impl Responder {
    let device = path.into_inner().0;
    if query.is_empty() {
        let redirect_url = format!("/{}/?0", device);
        return HttpResponse::MovedPermanently()
                .append_header(("Location", redirect_url))
                .finish();
    }
    let id = query.iter().next().map(|(_, value)| value.clone()).unwrap_or_else(|| "0".to_string());
    let id = if id.is_empty() { "0".to_string() } else { id };
    let id_num: i32 = match id.parse() {
        Ok(num) => num,
        Err(_) => return HttpResponse::BadRequest().body("Invalid id: not a number"),
    };
    let session_id = session.get::<String>("session_id").unwrap_or(None);
    if session_id.is_none() {
        session.insert("session_id", Uuid::new_v4().to_string()).unwrap();
    }
    let mut image_map = data.image_map.lock().unwrap();
    if let Some(image_url) = image_map.get(&id_num) {
        return HttpResponse::MovedPermanently()
            .append_header(("Location", image_url.as_str()))
            .finish();
    }
    let external_url = format!("https://t.alcy.cc/{}/?json", device);
    println!("{}", external_url);
    match client.get(&external_url).send().await {
        Ok(response) => match response.json::<ImageResponse>().await {
            Ok(image_response) => {
                if image_response.resp_code == 200 {
                    let image_url = image_response.image_url;
                    image_map.insert(id_num.clone(), image_url.clone());
                    HttpResponse::MovedPermanently()
                        .append_header(("Location", image_url.as_str()))
                        .finish()
                } else {
                    HttpResponse::InternalServerError().body("Invail response code from external server")
                }
            }
            Err(_) => HttpResponse::InternalServerError().body("Failed to parse response from external server")
        },
        Err(_) => HttpResponse::InternalServerError().body("Failed to send request to external server")
    }
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("debug"));

    let client = Client::new();
    let key = Key::generate();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                image_map: Mutex::new(HashMap::new()),
            }))
            .app_data(web::Data::new(client.clone()))
            .wrap(SessionMiddleware::new(
                actix_session::storage::CookieSessionStore::default(),
                key.clone(),
            ))
            .route("/{device}/", web::get().to(handle_request))
    })
        .bind("0.0.0.0:45123")?
        .run()
        .await
}
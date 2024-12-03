use std::collections::HashMap;
use actix_web::{web, HttpResponse, Responder, get, post, cookie, http::header, HttpRequest};
use bcrypt::{hash, verify};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::AppState;
use crate::models::{ChatMessage, SessionToken, User};

#[derive(Deserialize)]
pub struct RegisterData {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginData {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct ApiUser {
    pub id: i64,
    pub username: String,
}

// Форма реєстрації
#[get("/register")]
async fn register_form() -> HttpResponse {
    let html = include_str!("../static/register.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

// Форма входу
#[get("/login")]
async fn login_form() -> HttpResponse {
    let html = include_str!("../static/login.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

// Обробка реєстрації
#[post("/register")]
async fn register(
    data: web::Form<RegisterData>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let db = &app_state.db_pool;

    let hashed_password = hash(&data.password, 4).unwrap();
    let result = sqlx::query!(
        "INSERT INTO users (username, password) VALUES (?, ?)",
        data.username,
        hashed_password
    )
        .execute(db)
        .await;

    match result {
        Ok(_) => HttpResponse::Found()
            .insert_header((
                header::LOCATION,
                "/login?message=Реєстрація успішна&success=true",
            ))
            .finish(),
        Err(_) => HttpResponse::Found()
            .insert_header((
                header::LOCATION,
                "/register?message=Ім'я користувача вже зайнято&success=false",
            ))
            .finish(),
    }
}

// Обробка входу
#[post("/login")]
async fn login(
    data: web::Form<LoginData>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let db = &app_state.db_pool;

    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE username = ?"
    )
        .bind(&data.username)
        .fetch_optional(db)
        .await;

    if let Ok(Some(user)) = user {
        if verify(&data.password, &user.password).unwrap() {
            let session_token = Uuid::new_v4().to_string();
            sqlx::query!(
                "INSERT INTO sessions (user_id, session_token) VALUES (?, ?)",
                user.id,
                session_token
            )
                .execute(db)
                .await
                .unwrap();

            return HttpResponse::Found()
                .cookie(cookie::Cookie::build("session_token", session_token).finish())
                .insert_header((header::LOCATION, "/"))
                .finish();
        }
    }

    HttpResponse::Found()
        .insert_header((
            header::LOCATION,
            "/login?message=Невірні дані&success=false",
        ))
        .finish()
}

// Головна сторінка
#[get("/")]
async fn index(req: HttpRequest, app_state: web::Data<AppState>) -> HttpResponse {
    let db = &app_state.db_pool;

    if let Some(cookie) = req.cookie("session_token") {
        let session = sqlx::query_as::<_, SessionToken>(
            "SELECT * FROM sessions WHERE session_token = ?"
        )
            .bind(cookie.value())
            .fetch_optional(db)
            .await
            .unwrap();

        if session.is_some() {
            let html = include_str!("../static/index.html");
            return HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(html);
        }
    }

    HttpResponse::Found()
        .insert_header((header::LOCATION, "/login?message=Будь ласка, увійдіть у систему&success=false"))
        .finish()
}

// Обробка виходу
#[post("/logout")]
async fn logout() -> impl Responder {
    HttpResponse::Found()
        .insert_header((header::LOCATION, "/login?message=Вихід успішний&success=true"))
        .cookie(
            cookie::Cookie::build("session_token", "")
                .path("/")
                .max_age(cookie::time::Duration::seconds(0)) // Встановлення cookie, яке відразу видаляється
                .finish(),
        )
        .finish()
}

#[get("/api/users")]
async fn get_users(app_state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let db = &app_state.db_pool;

    // Отримуємо поточного користувача
    if let Some(cookie) = req.cookie("session_token") {
        let session = sqlx::query_as::<_, SessionToken>(
            "SELECT * FROM sessions WHERE session_token = ?",
        )
            .bind(cookie.value())
            .fetch_optional(db)
            .await
            .unwrap();

        if let Some(session) = session {
            let user_id = session.user_id;

            // Отримуємо список користувачів, виключаючи поточного
            let users = sqlx::query!(
                "SELECT id, username FROM users WHERE id != ?",
                user_id
            )
                .fetch_all(db)
                .await
                .unwrap_or_default();

            let users: Vec<ApiUser> = users.into_iter()
                .map(|record| ApiUser {
                    id: record.id,
                    username: record.username,
                })
                .collect();

            return HttpResponse::Ok().json(users);
        }
    }

    HttpResponse::Unauthorized().body("Unauthorized")
}

#[get("/api/messages")]
async fn get_messages(
    app_state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let db = &app_state.db_pool;

    if let Some(cookie) = req.cookie("session_token") {
        let session = sqlx::query_as::<_, SessionToken>(
            "SELECT * FROM sessions WHERE session_token = ?",
        )
            .bind(cookie.value())
            .fetch_optional(db)
            .await
            .unwrap();

        if let Some(session) = session {
            let user_id = session.user_id;

            if let Some(recipient_id_str) = query.get("recipient_id") {
                if let Ok(recipient_id) = recipient_id_str.parse::<i64>() {
                    // Отримуємо повідомлення між поточним користувачем і обраним одержувачем
                    let messages = sqlx::query!(
                        "SELECT
                            messages.id,
                            messages.sender_id,
                            messages.recipient_id,
                            users.username AS sender_name,
                            messages.content,
                            messages.timestamp
                        FROM
                            messages
                        JOIN
                            users ON messages.sender_id = users.id
                        WHERE
                            (messages.sender_id = ? AND messages.recipient_id = ?)
                            OR
                            (messages.sender_id = ? AND messages.recipient_id = ?)
                        ORDER BY
                            messages.timestamp ASC;",
                        user_id,
                        recipient_id,
                        recipient_id,
                        user_id
                    )
                        .fetch_all(db)
                        .await
                        .unwrap_or_default();

                    let messages: Vec<ChatMessage> = messages.into_iter()
                        .filter_map(|record| {
                            Some(ChatMessage {
                                id: record.id,
                                sender_id: record.sender_id,
                                recipient_id: record.recipient_id.expect("REASON"),
                                sender_name: record.sender_name,
                                content: record.content,
                                timestamp: record.timestamp.unwrap_or_else(|| "Unknown".to_string()),
                            })
                        })
                        .collect();


                    return HttpResponse::Ok().json(messages);
                }
            }
        }
    }

    HttpResponse::Unauthorized().body("Unauthorized")
}
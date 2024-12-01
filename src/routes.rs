use actix_web::{web, HttpResponse, Responder, get, post, cookie, http::header, HttpRequest};
use bcrypt::{hash, verify};
use serde::Deserialize;
use sqlx::SqlitePool;
use uuid::Uuid;
use crate::models::{SessionToken, User};

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
async fn register(data: web::Form<RegisterData>, db: web::Data<SqlitePool>) -> HttpResponse {
    let hashed_password = hash(&data.password, 4).unwrap();
    let result = sqlx::query!(
        "INSERT INTO users (username, password) VALUES (?, ?)",
        data.username,
        hashed_password
    )
        .execute(db.get_ref())
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
async fn login(data: web::Form<LoginData>, db: web::Data<SqlitePool>) -> HttpResponse {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&data.username)
        .fetch_optional(db.get_ref())
        .await;

    if let Ok(Some(user)) = user {
        if verify(&data.password, &user.password).unwrap() {
            let session_token = Uuid::new_v4().to_string();
            sqlx::query!(
                "INSERT INTO sessions (user_id, session_token) VALUES (?, ?)",
                user.id,
                session_token
            )
                .execute(db.get_ref())
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

#[get("/")]
async fn index(req: HttpRequest, db: web::Data<SqlitePool>) -> HttpResponse {
    if let Some(cookie) = req.cookie("session_token") {
        let session = sqlx::query_as::<_, SessionToken>(
            "SELECT * FROM sessions WHERE session_token = ?"
        )
            .bind(cookie.value())
            .fetch_optional(db.get_ref())
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

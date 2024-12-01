mod models;
mod websocket;
mod routes;

use actix_web::{web, App, HttpServer};
use sqlx::SqlitePool;


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let db_pool = SqlitePool::connect("sqlite://app.db").await.unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(db_pool.clone()))
            .service(routes::register_form)
            .service(routes::login_form)
            .service(routes::register)
            .service(routes::login)
            .service(routes::index)
            .service(routes::logout)
            .service(web::resource("/ws/").route(web::get().to(websocket::websocket_route)))
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}

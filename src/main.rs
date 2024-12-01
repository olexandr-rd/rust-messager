use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder, get};
use actix_ws::{Message, Session};
use futures_util::{StreamExt, SinkExt};

// Обробка WebSocket-з'єднань
async fn websocket_handler(
    mut session: Session,
    mut msg_stream: actix_ws::MessageStream,
) -> Result<(), Error> {
    while let Some(Ok(msg)) = msg_stream.next().await {
        match msg {
            Message::Text(text) => {
                if let Err(err) = session.text(format!("You said: {}", text)).await {
                    eprintln!("Failed to send message: {:?}", err);
                }
            }
            Message::Close(reason) => {
                // Закриваємо з'єднання
                if let Err(err) = session.close(reason).await {
                    eprintln!("Failed to close connection: {:?}", err);
                }
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

// Головна сторінка
#[get("/")]
async fn index() -> impl Responder {
    let html = include_str!("../static/index.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

// WebSocket маршрут
#[get("/ws/")]
async fn ws_handler(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let (res, session, msg_stream) = actix_ws::handle(&req, stream)?;
    // Запускаємо WebSocket-обробник у асинхронному контексті
    actix_web::rt::spawn(websocket_handler(session, msg_stream));
    Ok(res)
}

// Головна функція
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(index)     // Маршрут для головної сторінки
            .service(ws_handler) // Маршрут для WebSocket-з'єднання
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}

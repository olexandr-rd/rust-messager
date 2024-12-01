use actix_web::{web, HttpResponse, HttpRequest, Error, get, rt};
use actix_ws::{Message, MessageStream, Session, handle};
use sqlx::SqlitePool;
use futures_util::StreamExt;
use crate::models::SessionToken;
use crate::models::ChatMessage;

// WebSocket обробник
#[get("/ws/")]
async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    db: web::Data<SqlitePool>,
) -> Result<HttpResponse, Error> {
    if let Some(cookie) = req.cookie("session_token") {
        let session = sqlx::query_as::<_, SessionToken>(
            "SELECT * FROM sessions WHERE session_token = ?",
        )
            .bind(cookie.value())
            .fetch_optional(db.get_ref())
            .await
            .unwrap();

        if let Some(session) = session {
            let user_id = session.user_id;

            // Отримуємо історію повідомлень
            let messages = sqlx::query_as::<_, ChatMessage>(
                "SELECT * FROM messages ORDER BY timestamp ASC"
            )
                .fetch_all(db.get_ref())
                .await
                .unwrap_or_else(|_| vec![]);

            // Створюємо WebSocket з'єднання
            let (res, mut session, msg_stream) = handle(&req, stream)?;

            // Відправляємо історію повідомлень
            for message in messages {
                let formatted_message = format!("{}: {}", message.sender_id, message.content);
                if let Err(err) = session.text(formatted_message).await {
                    eprintln!("Failed to send message history: {:?}", err);
                }
            }

            rt::spawn(websocket_handler(session, msg_stream, db.clone(), user_id));
            return Ok(res);
        }
    }

    Ok(HttpResponse::Unauthorized().body("Unauthorized"))
}

// Обробник WebSocket-повідомлень
async fn websocket_handler(
    mut session: Session,
    mut msg_stream: MessageStream,
    db: web::Data<SqlitePool>,
    sender_id: i64,
) -> Result<(), Error> {
    println!("WebSocket handler started for sender_id: {}", sender_id);

    while let Some(Ok(msg)) = msg_stream.next().await {
        println!("Received message: {:?}", msg); // Діагностика

        match msg {
            Message::Text(text) => {
                let text = text.to_string(); // Конвертуємо в `String`
                let text = text.as_str();    // Конвертуємо в `&str`

                // Збереження повідомлення у базу
                let result = sqlx::query!(
                    "INSERT INTO messages (sender_id, content) VALUES (?, ?)",
                    sender_id,
                    text
                )
                    .execute(db.get_ref())
                    .await;

                if let Err(err) = result {
                    eprintln!("Failed to save message: {:?}", err);
                }

                // Отримуємо ім'я відправника
                let sender_name = sqlx::query_scalar!(
                    "SELECT username FROM users WHERE id = ?",
                    sender_id
                )
                    .fetch_one(db.get_ref())
                    .await
                    .unwrap_or_else(|_| "Анонім".to_string());

                // Отримуємо поточний таймстемп
                let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

                // Форматування повідомлення
                let formatted_message = format!(
                    "{}: {}      [{}]",
                    sender_name, text, timestamp
                );
                println!("Sending formatted message: {}", formatted_message);

                // Відправлення повідомлення клієнту
                if let Err(err) = session.text(formatted_message).await {
                    eprintln!("Failed to send message: {:?}", err);
                }
            }
            Message::Close(reason) => {
                println!("Connection closed: {:?}", reason);

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


// WebSocket маршрут
pub async fn websocket_route(
    req: HttpRequest,
    stream: web::Payload,
    db: web::Data<SqlitePool>,
) -> Result<HttpResponse, Error> {
    println!("WebSocket route triggered"); // Діагностика

    if let Some(cookie) = req.cookie("session_token") {
        println!("Cookie found: {:?}", cookie); // Діагностика

        let session = sqlx::query_as::<_, SessionToken>(
            "SELECT * FROM sessions WHERE session_token = ?",
        )
            .bind(cookie.value())
            .fetch_optional(db.get_ref())
            .await;

        match session {
            Ok(Some(session)) => {
                println!("Session found for user_id: {}", session.user_id);

                let user_id = session.user_id;

                // Отримання історії повідомлень
                let messages = sqlx::query_as::<_, ChatMessage>(
                    "SELECT
                        messages.id,
                        messages.sender_id,
                        users.username AS sender_name,
                        messages.content,
                        messages.timestamp
                    FROM
                        messages
                    JOIN
                        users
                    ON
                        messages.sender_id = users.id
                    ORDER BY
                        messages.timestamp ASC;"
                )
                    .fetch_all(db.get_ref())
                    .await;

                match messages {
                    Ok(messages) => {
                        println!("Messages retrieved: {}", messages.len());

                        let (response, mut session, msg_stream) = handle(&req, stream)?;

                        // Надсилання історії повідомлень
                        for message in messages {
                            let formatted_message = format!(
                                "{}: {}      [{}]",
                                message.sender_name, message.content, message.timestamp
                            );
                            println!("Sending message: {}", formatted_message); // Діагностика

                            if let Err(err) = session.text(formatted_message).await {
                                eprintln!("Failed to send message history: {:?}", err);
                            }
                        }

                        // Запуск обробника повідомлень
                        rt::spawn(websocket_handler(session, msg_stream, db.clone(), user_id));
                        return Ok(response);
                    }
                    Err(err) => {
                        eprintln!("Failed to fetch messages: {:?}", err);
                    }
                }
            }
            Ok(None) => {
                println!("Session not found");
            }
            Err(err) => {
                eprintln!("Failed to fetch session: {:?}", err);
            }
        }
    }

    Ok(HttpResponse::Unauthorized().body("Unauthorized"))
}



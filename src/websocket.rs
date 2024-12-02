use actix_web::{web, HttpRequest, HttpResponse, Error};
use actix_ws::{handle, Message, MessageStream, Session};
use futures_util::StreamExt;
use crate::models::{SessionToken, ChatMessage};
use crate::AppState;

pub async fn websocket_route(
    req: HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
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

            // Отримання історії повідомлень
            let messages = sqlx::query_as::<_, ChatMessage>(
                "SELECT
                    messages.id AS id,
                    messages.sender_id AS sender_id,
                    users.username AS sender_name,
                    messages.content AS content,
                    messages.timestamp AS timestamp
                FROM
                    messages
                JOIN
                    users
                ON
                    messages.sender_id = users.id
                ORDER BY
                    messages.timestamp ASC;"
            )
                .fetch_all(db)
                .await
                .unwrap_or_else(|_| vec![]);

            // match messages {
            //     Ok(messages) => {
            //         println!("Messages retrieved: {:?}", messages);
            //     }
            //     Err(err) => {
            //         eprintln!("Failed to fetch messages: {:?}", err);
            //     }
            // }

            // Створення WebSocket з'єднання
            let (response, mut session, msg_stream) = handle(&req, stream)?;

            // Надсилання історії повідомлень
            for message in messages {
                let formatted_message = format!(
                    "{}: {}      [{}]",
                    message.sender_name, message.content, message.timestamp
                );
                if let Err(err) = session.text(formatted_message).await {
                    eprintln!("Failed to send message history: {:?}", err);
                }
            }

            // Запуск обробника повідомлень
            actix_web::rt::spawn(websocket_handler(
                session,
                msg_stream,
                app_state.clone(),
                user_id,
            ));
            return Ok(response);
        }
    }

    Ok(HttpResponse::Unauthorized().body("Unauthorized"))
}

async fn websocket_handler(
    mut session: Session,
    mut msg_stream: MessageStream,
    app_state: web::Data<AppState>,
    sender_id: i64,
) -> Result<(), Error> {
    let db = &app_state.db_pool;
    let mut rx = app_state.tx.subscribe();

    loop {
        tokio::select! {
            Some(Ok(msg)) = msg_stream.next() => {
                match msg {
                    Message::Text(text) => {
                        let text = text.to_string();

                        // Збереження повідомлення у базу
                        let result = sqlx::query!(
                            "INSERT INTO messages (sender_id, content) VALUES (?, ?)",
                            sender_id,
                            text
                        )
                        .execute(db)
                        .await;

                        if let Err(err) = result {
                            eprintln!("Failed to save message: {:?}", err);
                        }

                        // Отримуємо ім'я відправника
                        let sender_name = sqlx::query_scalar!(
                            "SELECT username FROM users WHERE id = ?",
                            sender_id
                        )
                        .fetch_one(db)
                        .await
                        .unwrap_or_else(|_| "Анонім".to_string());

                        // Отримуємо поточний таймстемп
                        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

                        // Форматування повідомлення
                        let formatted_message = format!(
                            "{}: {}      [{}]",
                            sender_name, text, timestamp
                        );

                        // Надсилання повідомлення всім клієнтам через канал мовлення
                        let _ = app_state.tx.send(formatted_message);
                    }
                    Message::Close(reason) => {
                        if let Err(err) = session.close(reason).await {
                            eprintln!("Failed to close connection: {:?}", err);
                        }
                        break;
                    }
                    _ => {}
                }
            }
            Ok(message) = rx.recv() => {
                // Надсилання повідомлення клієнту
                if let Err(err) = session.text(message).await {
                    eprintln!("Failed to send message to client: {:?}", err);
                }
            }
            else => {
                break;
            }
        }
    }

    Ok(())
}

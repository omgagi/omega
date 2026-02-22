//! QR code generation and standalone pairing flow.

use crate::whatsapp_store::SqlxWhatsAppStore;
use omega_core::error::OmegaError;
use std::sync::Arc;
use tokio::sync::mpsc;
use wacore::types::events::Event;
use whatsapp_rust::bot::Bot;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

/// Generate a compact QR code for terminal display using Unicode half-block characters.
///
/// Packs two rows of modules into one line of text using `▀`, `▄`, `█`, and space.
/// This produces a QR code roughly half the height of a naive renderer.
pub fn generate_qr_terminal(qr_data: &str) -> Result<String, OmegaError> {
    use qrcode::{Color, EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(qr_data.as_bytes(), EcLevel::L)
        .map_err(|e| OmegaError::Channel(format!("QR generation failed: {e}")))?;

    let width = code.width();
    let colors: Vec<Color> = code.into_colors();
    let is_dark = |row: usize, col: usize| -> bool {
        if row < width && col < width {
            colors[row * width + col] == Color::Dark
        } else {
            false
        }
    };

    let mut out = String::new();
    // Process two rows at a time.
    let mut row = 0;
    while row < width {
        for col in 0..width {
            let top = is_dark(row, col);
            let bottom = if row + 1 < width {
                is_dark(row + 1, col)
            } else {
                false
            };
            out.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push('\n');
        row += 2;
    }

    Ok(out)
}

/// Generate a QR code as PNG image bytes (for sending as a photo).
pub fn generate_qr_image(qr_data: &str) -> Result<Vec<u8>, OmegaError> {
    use image::{ImageBuffer, Luma};
    use qrcode::{EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(qr_data.as_bytes(), EcLevel::L)
        .map_err(|e| OmegaError::Channel(format!("QR generation failed: {e}")))?;

    let module_size: u32 = 10;
    let quiet_zone: u32 = 2;
    let modules = code.width() as u32;
    let img_size = (modules + quiet_zone * 2) * module_size;

    let img = ImageBuffer::from_fn(img_size, img_size, |x, y| {
        let mx = (x / module_size).saturating_sub(quiet_zone);
        let my = (y / module_size).saturating_sub(quiet_zone);

        if x / module_size < quiet_zone
            || y / module_size < quiet_zone
            || mx >= modules
            || my >= modules
        {
            Luma([255u8]) // White border
        } else {
            use qrcode::Color;
            match code[(mx as usize, my as usize)] {
                Color::Dark => Luma([0u8]),
                Color::Light => Luma([255u8]),
            }
        }
    });

    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| OmegaError::Channel(format!("PNG encoding failed: {e}")))?;

    Ok(buf.into_inner())
}

/// Start the pairing flow: returns an mpsc receiver that yields QR data strings
/// as WhatsApp rotates QR codes periodically.
pub async fn start_pairing(
    data_dir: &str,
) -> Result<(mpsc::Receiver<String>, mpsc::Receiver<bool>), OmegaError> {
    let (qr_tx, qr_rx) = mpsc::channel::<String>(4);
    let (done_tx, done_rx) = mpsc::channel::<bool>(1);

    let dir = omega_core::config::shellexpand(data_dir);
    let session_dir = format!("{dir}/whatsapp_session");
    let _ = std::fs::create_dir_all(&session_dir);
    let db_path = format!("{session_dir}/whatsapp.db");

    let backend = Arc::new(
        SqlxWhatsAppStore::new(&db_path)
            .await
            .map_err(|e| OmegaError::Channel(format!("whatsapp store init failed: {e}")))?,
    );

    let mut bot = Bot::builder()
        .with_backend(backend)
        .with_transport_factory(TokioWebSocketTransportFactory::new())
        .with_http_client(UreqHttpClient::new())
        .with_device_props(
            Some("OMEGA".to_string()),
            None,
            Some(waproto::whatsapp::device_props::PlatformType::Desktop),
        )
        .on_event(move |event, _client| {
            let qr_tx = qr_tx.clone();
            let done_tx = done_tx.clone();
            async move {
                match event {
                    Event::PairingQrCode { code, .. } => {
                        let _ = qr_tx.send(code).await;
                    }
                    Event::PairSuccess(_) | Event::Connected(_) => {
                        let _ = done_tx.send(true).await;
                    }
                    _ => {}
                }
            }
        })
        .build()
        .await
        .map_err(|e| OmegaError::Channel(format!("whatsapp pairing build failed: {e}")))?;

    let _handle = bot
        .run()
        .await
        .map_err(|e| OmegaError::Channel(format!("whatsapp pairing run failed: {e}")))?;

    Ok((qr_rx, done_rx))
}

# Functionalities: Channels

## Overview
Two messaging platform integrations (Telegram and WhatsApp) behind a common `Channel` trait. Both support text, voice (via Whisper transcription), and image attachments. WhatsApp uses the WhatsApp Web protocol with QR pairing.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | Channel trait | Trait | backend/crates/omega-core/src/traits.rs:~20 | name(), start(tx), send(text, target), send_typing(target), send_photo(path, target), stop(), as_any() | -- |
| 2 | TelegramChannel | Struct | backend/crates/omega-channels/src/telegram/mod.rs:18 | Bot API with long polling via getUpdates; voice transcription via Whisper; photo attachment download | Whisper |
| 3 | WhatsAppChannel | Struct | backend/crates/omega-channels/src/whatsapp/mod.rs:26 | WhatsApp Web protocol (Noise + Signal encryption); QR pairing; session persistence to whatsapp.db; voice/image support | whatsapp-rust, Whisper |
| 4 | WhatsAppChannel::pairing_channels() | Method | backend/crates/omega-channels/src/whatsapp/mod.rs:71 | Creates mpsc channels for QR/done events; replays buffered QR code | -- |
| 5 | transcribe_whisper() | Function | backend/crates/omega-channels/src/whisper.rs:13 | OpenAI Whisper API: audio bytes -> text transcription | OpenAI API |
| 6 | generate_qr_image() | Function | backend/crates/omega-channels/src/whatsapp/qr.rs | Generates QR code as PNG bytes for WhatsApp pairing | -- |
| 7 | generate_qr_terminal() | Function | backend/crates/omega-channels/src/whatsapp/qr.rs | Generates QR code for terminal display | -- |
| 8 | start_pairing() | Function | backend/crates/omega-channels/src/whatsapp/qr.rs | Initiates WhatsApp pairing process | WhatsApp channel |

## Internal Dependencies
- TelegramChannel.start() -> long polling loop -> mpsc -> Gateway
- WhatsAppChannel.start() -> WhatsApp Web connection -> mpsc -> Gateway
- Voice messages -> transcribe_whisper() -> text handling
- WhatsApp pairing -> QR generation -> API/Telegram delivery

## Dead Code / Unused
None detected.

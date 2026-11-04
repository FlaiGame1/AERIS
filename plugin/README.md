# AERIS Plugin

**AERIS Plugin 1.0.0** is the Android-side component of the AERIS project. It is a Python plugin for compatible **AyuGram / exteraGram** clients. It is not a standalone Android application and does not have a separate APK.

## Features

- Adds **“Play through AERIS” / «Воспроизвести через AERIS»** to the message long-press context menu.
- Sends supported audio and video files from Telegram to AERIS Desktop over the local network.
- Uses the active Telegram client's FileLoader to locate downloaded media instead of assuming a fixed cache directory.
- Pairs with the Windows receiver using a six-character temporary code.
- Reuses the saved device token after successful pairing.
- Provides playback controls and playback status through the plugin settings.
- Includes best-effort integrations with the audio-player overflow menu and profile music.

## Installation

1. Install a compatible AyuGram or exteraGram client with Python plugin support.
2. Install the AERIS plugin using that client's plugin manager.
3. Start **AERIS Desktop** on your Windows PC.
4. Connect the phone and PC to the same trusted Wi-Fi/LAN.
5. Open AERIS plugin settings and enter the PC's local IP address and port **45872**.
6. Enter the six-character pairing code displayed by AERIS Desktop.
7. In Telegram, long-press a downloaded audio or video message and choose **“Play through AERIS” / «Воспроизвести через AERIS»**.

After the first successful pairing, the plugin reuses the saved device token.

## Media and Telegram limitations

- The media must already be downloaded and exposed by Telegram's FileLoader as a readable local file.
- The plugin does not independently decrypt Telegram cache files and does not bypass Telegram download restrictions.
- If Telegram cannot download the media, use the browser uploader provided by AERIS Desktop: `http://<PC-LAN-IP>:45873/`. Open this address on the phone and select a file from Android storage.
- The **three-dot menu on an individual video message is not integrated yet**. Use the message long-press context menu.
- Audio-player and profile-music integrations depend on internal client APIs and may behave differently between AyuGram / exteraGram versions.

## Network and security

AERIS transfers media directly between the phone and PC over the local network. Telegram servers are not used as the media relay for this transfer.

The current LAN transport is **not encrypted**. Use AERIS only on a trusted private network. Do not expose ports **45872** or **45873** to the public internet.

## Compatibility

The plugin requires a compatible AyuGram / exteraGram client with the plugin SDK and the FileLoader bridge used by AERIS. It is not intended to run as a regular Android app.

## Protocol

AERIS Desktop and the Android plugin use the shared AERIS protocol:

- [Protocol specification](../protocol/PROTOCOL.md)
- [Main project README](../README.md)

---

# Плагин AERIS

**AERIS Plugin 1.0.0** — Android-компонент проекта AERIS. Это Python-плагин для совместимых клиентов **AyuGram / exteraGram**. Это не отдельное Android-приложение, APK для него не выпускается.

## Возможности

- Добавляет пункт **«Воспроизвести через AERIS»** в контекстное меню сообщения при долгом нажатии.
- Передаёт поддерживаемые аудио- и видеофайлы из Telegram на AERIS Desktop по локальной сети.
- Использует FileLoader установленного клиента Telegram для поиска загруженного медиа, а не предполагает фиксированный путь к кэшу.
- Подключается к Windows-приёмнику по временному шестизначному коду.
- После первого успешного сопряжения повторно использует сохранённый токен устройства.
- Предоставляет управление воспроизведением и его статус в настройках плагина.
- Содержит экспериментальные интеграции с меню аудиоплеера и музыкой профиля.

## Установка и настройка

1. Установи совместимый AyuGram или exteraGram с поддержкой Python-плагинов.
2. Установи плагин AERIS через менеджер плагинов клиента.
3. Запусти **AERIS Desktop** на компьютере с Windows.
4. Подключи телефон и компьютер к одной доверенной сети Wi-Fi/LAN.
5. Открой настройки плагина AERIS и укажи локальный IP компьютера и порт **45872**.
6. Введи шестизначный код сопряжения, который показывает AERIS Desktop.
7. В Telegram зажми уже загруженное аудио- или видеосообщение и выбери **«Воспроизвести через AERIS»**.

После первого успешного сопряжения плагин будет повторно использовать сохранённый токен устройства.

## Ограничения Telegram и медиа

- Файл должен быть предварительно загружен и доступен FileLoader Telegram как обычный локальный файл.
- Плагин самостоятельно не расшифровывает файлы из кэша Telegram и не обходит ограничения на скачивание.
- Если Telegram не может загрузить медиа, воспользуйся браузерным загрузчиком AERIS Desktop: `http://<IP-КОМПЬЮТЕРА>:45873/`. Открой адрес на телефоне и выбери файл из памяти Android.
- **Пункт в меню трёх точек отдельного видеосообщения пока не реализован.** Используй контекстное меню при долгом нажатии на сообщение.
- Интеграции с аудиоплеером и музыкой профиля зависят от внутренних API клиента и могут отличаться между версиями AyuGram / exteraGram.

## Сеть и безопасность

AERIS передаёт медиа напрямую между телефоном и компьютером по локальной сети. Серверы Telegram не выступают посредником при передаче самого файла.

Текущий транспорт в локальной сети **не зашифрован**. Используй AERIS только в доверенной частной сети. Не открывай порты **45872** и **45873** в публичный интернет.

## Совместимость

Плагину нужен совместимый клиент AyuGram / exteraGram с SDK Python-плагинов и используемым AERIS мостом к FileLoader. Плагин не предназначен для запуска как обычное Android-приложение.

## Протокол

Windows-клиент и Android-плагин используют общий протокол AERIS:

- [Спецификация протокола](../protocol/PROTOCOL.md)
- [Основной README проекта](../README.md)

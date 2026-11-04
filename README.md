# AERIS

**Open-source media casting ecosystem / Открытая экосистема трансляции медиа**

[English](#english) · [Русский](#русский)

---

## English

AERIS sends media from Android to a Windows PC over the local network. It does not use Google Cast or route media through Telegram servers.

The repository contains two components:

- **AERIS Desktop** — Windows receiver and lightweight audio player, written in Rust.
- **AERIS Plugin** — Python plugin for AyuGram / exteraGram that sends cached Telegram media and controls playback.

Both components communicate through the AERIS Protocol over a direct WebSocket connection on the local network (default TCP port **45872**).

### Current capabilities

- Pair a phone with the Windows receiver using a six-character code.
- Transfer cached media in 1 MiB chunks, up to 512 MiB per file.
- Verify file size and SHA-256 before playback.
- Play supported audio formats inside AERIS Desktop (MP3, WAV, FLAC, OGG, M4A/AAC and AIFF/ALAC where supported by the decoder).
- Open video and unsupported formats with the Windows default application.
- Control AERIS audio from the phone: resume, pause, stop, seek and volume.
- Control playback locally from the Desktop window.

### Telegram availability and network behavior

The AERIS connection is **phone ↔ PC over your private LAN**. It does not pass through Telegram, Telegram proxies, or Telegram servers. Telegram blocking or an outage therefore does not by itself interrupt an established AERIS LAN connection.

There is one separate limitation: the plugin currently reads media from Telegram's local cache. The media must already be downloaded on the phone. If Telegram cannot download a file because of blocking or connectivity problems, AERIS cannot transfer that file until it is available locally. Direct downloading of uncached Telegram media is not implemented.

### Pairing and security

The pairing code is generated when the Desktop receiver starts and remains valid until the receiver is stopped. Restarting the receiver generates a new code. This allows the plugin to reconnect for each playback-control action without requiring a new code every three minutes.

The current transport is unencrypted ws://. Use AERIS only on a trusted private network. Do not expose port 45872 to the public internet. Encrypted transport, persistent device credentials and pairing revocation remain future work.

### Current limitations

- Native in-app playback currently targets audio. Video is transferred and opened by the Windows default media application; remote video controls are not yet integrated.
- The plugin can only transfer files already present in Telegram's local cache.
- The built-in Telegram media-player overflow menu and profile music integration are not implemented.
- Playback status synchronization and a persistent device-management screen are planned.

### Repository structure

    AERIS/
    ├── desktop/          Rust Windows receiver and audio player
    ├── plugin/           Python AyuGram / exteraGram plugin
    ├── protocol/         Shared protocol specification
    ├── docs/             Architecture and developer documentation
    ├── licenses/         Third-party license notices
    └── .github/workflows CI and release builds

### Development

AERIS is an early preview project. Bug reports and contributions are welcome. When reporting issues, include the Windows version, Android client and plugin versions, the AERIS Desktop log, and whether the media was fully downloaded in Telegram.

---

## Русский

AERIS передаёт медиа с Android на компьютер с Windows через локальную сеть. Проект не использует Google Cast и не отправляет медиа через серверы Telegram.

В репозитории находятся два компонента:

- **AERIS Desktop** — приёмник для Windows и лёгкий аудиоплеер на Rust.
- **AERIS Plugin** — Python-плагин для AyuGram / exteraGram, который отправляет закэшированные медиа из Telegram и управляет воспроизведением.

Компоненты общаются через протокол AERIS по прямому WebSocket-соединению внутри локальной сети (стандартный TCP-порт **45872**).

### Что уже работает

- Сопряжение телефона с компьютером по шестизначному коду.
- Передача закэшированных медиа блоками по 1 МиБ, размер файла — до 512 МиБ.
- Проверка размера файла и SHA-256 перед воспроизведением.
- Воспроизведение поддерживаемых аудиоформатов внутри AERIS Desktop (MP3, WAV, FLAC, OGG, Opus, M4A/AAC и AIFF/ALAC при поддержке декодером).
- Передача видео и неподдерживаемых форматов в стандартное приложение Windows.
- Управление аудио с телефона: продолжить, пауза, остановка, перемотка и громкость.
- Локальное управление плеером из окна Desktop.

### Блокировка Telegram и работа сети

Соединение AERIS строится **напрямую между телефоном и ПК по локальной сети**. Оно не проходит через Telegram, Telegram Proxy или серверы Telegram. Поэтому блокировка или сбой Telegram сами по себе не должны прерывать уже установленное LAN-соединение AERIS.

Но есть отдельное ограничение: плагин пока получает медиа из локального кэша Telegram. Файл должен быть предварительно загружен на телефон. Если Telegram не может скачать его из-за блокировки или проблем с интернетом, AERIS не сможет передать этот файл, пока он не появится локально. Автоматическая загрузка незакэшированных файлов пока не реализована.

### Сопряжение и безопасность

Код сопряжения генерируется при запуске Desktop-приёмника и действует до его остановки. При перезапуске создаётся новый код. Благодаря этому плагин может заново подключаться для каждой команды управления, не требуя вводить новый код каждые три минуты.

Текущий транспорт использует незашифрованный ws://. Используй AERIS только в доверенной частной сети. Не открывай порт 45872 в публичный интернет. Шифрование, постоянные учётные данные устройств и отзыв сопряжения остаются задачами на будущее.

### Текущие ограничения

- Встроенное воспроизведение пока предназначено для аудио. Видео передаётся и открывается стандартным приложением Windows; удалённое управление видеоплеером ещё не подключено.
- Плагин может передавать только файлы, уже находящиеся в локальном кэше Telegram.
- Интеграция в меню трёх точек встроенного медиаплеера Telegram и в музыку профиля пока не реализована.
- Синхронизация состояния воспроизведения и отдельный экран управления подключёнными устройствами запланированы на будущее.

### Структура репозитория

    AERIS/
    ├── desktop/          Приёмник и аудиоплеер Windows на Rust
    ├── plugin/           Python-плагин AyuGram / exteraGram
    ├── protocol/         Спецификация общего протокола
    ├── docs/             Архитектура и документация разработчика
    ├── licenses/         Уведомления о лицензиях сторонних компонентов
    └── .github/workflows CI и сборки релизов

### Разработка

AERIS пока находится на стадии раннего preview. Приветствуются сообщения об ошибках и вклад в разработку. При описании проблемы укажи версию Windows, Android-клиента и плагина, приложи лог AERIS Desktop и уточни, был ли файл полностью загружен в Telegram.

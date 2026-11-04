# AERIS Android Companion

Standalone Android sender and remote for AERIS Desktop.

## Build

1. Install Android Studio and JDK 17.
2. Open this `android/` directory as a project.
3. Allow Android Gradle Plugin to install Android SDK 35 if prompted.
4. Select the `app` configuration and build a debug APK, or run:

```powershell
gradle assembleDebug
```

The APK is written to `app/build/outputs/apk/debug/app-debug.apk`.

## Use

1. Start AERIS Desktop on Windows and press **Запустить сервер**.
2. Put the phone and PC on the same trusted Wi-Fi network.
3. Enter the PC's LAN IP and port 45872.
4. On first connection, enter the six-character pairing code shown by Desktop. The app remembers the device token after pairing.
5. Choose a file using the Android file picker, or use **Поделиться → AERIS** in another app.
6. Press **Отправить на ПК и воспроизвести**. The desktop receives the file and starts mpv.
7. Use the controls in the app to pause, resume, stop, seek, change volume and inspect playback status.

## Network and privacy

The transfer is direct over the local network using the same AERIS WebSocket v1 protocol. No Telegram or Google account is required. Telegram connectivity is not involved when sending files from this app.

The current protocol uses unencrypted `ws://` and pairing tokens stored in Android app preferences. Use only a trusted private network. Do not expose TCP port 45872 to the public internet.

## Limits

- Maximum file size: 512 MiB.
- The phone and PC must be able to reach each other on the same LAN.
- The PC must have the bundled `mpv/` runtime beside `aeris-desktop.exe`.
- This is an early debug companion build; it is not yet published as a signed Play Store APK.

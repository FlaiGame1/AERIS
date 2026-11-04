package app.aeris

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import org.json.JSONObject
import java.io.InputStream
import java.security.MessageDigest
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

class MainActivity : Activity() {
    private lateinit var host: EditText
    private lateinit var port: EditText
    private lateinit var code: EditText
    private lateinit var status: TextView
    private var selectedUri: Uri? = null
    private val prefs by lazy { getSharedPreferences("aeris", MODE_PRIVATE) }
    private val client by lazy { OkHttpClient.Builder().connectTimeout(10, TimeUnit.SECONDS).readTimeout(0, TimeUnit.MILLISECONDS).build() }
    private val pickFile = 1001

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        buildUi()
        if (intent?.action == Intent.ACTION_SEND) {
            @Suppress("DEPRECATION")
            val shared = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
            if (shared != null) {
                selectedUri = shared
                status.text = "Получен файл из другого приложения. Нажми «Отправить на ПК»."
            }
        }
    }

    private fun buildUi() {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(24, 28, 24, 24)
            setBackgroundColor(0xFF121923.toInt())
        }
        val scroll = ScrollView(this)
        val content = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        fun label(value: String, size: Float = 15f) {
            content.addView(TextView(this).apply {
                text = value; textSize = size; setTextColor(0xFFF2F6FA.toInt())
                setPadding(0, 12, 0, 8)
            })
        }
        fun field(hint: String, value: String): EditText {
            val edit = EditText(this).apply {
                this.hint = hint; setSingleLine(true); setText(value)
                setTextColor(0xFFFFFFFF.toInt()); setHintTextColor(0xFF9BAABD.toInt())
                setPadding(14, 8, 14, 8)
                setBackgroundColor(0xFF263548.toInt())
            }
            content.addView(edit, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 52).apply { bottomMargin = 8 })
            return edit
        }
        fun button(title: String, action: () -> Unit) {
            content.addView(Button(this).apply {
                text = title; setOnClickListener { action() }
            }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 54).apply { topMargin = 6 })
        }
        label("AERIS", 28f)
        label("Передача медиа напрямую с Android на Windows по локальной сети.")
        host = field("IP компьютера", prefs.getString("host", "192.168.1.100") ?: "192.168.1.100")
        port = field("Порт", prefs.getString("port", "45872") ?: "45872")
        code = field("Код сопряжения (только первый раз)", "")
        label("Отправка")
        button("Выбрать файл на телефоне") {
            startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE); type = "*/*"
            }, pickFile)
        }
        button("Отправить на ПК и воспроизвести") { selectedUri?.let { transferAsync(it) } ?: run { status.text = "Сначала выбери файл или отправь его через «Поделиться»." } }
        label("Управление плеером")
        button("▶ Продолжить") { control("play") }
        button("Ⅱ Пауза / продолжить") { control("toggle_pause") }
        button("■ Стоп") { control("stop") }
        button("↶ −10 секунд") { control("seek_relative", JSONObject().put("offset", -10)) }
        button("↷ +10 секунд") { control("seek_relative", JSONObject().put("offset", 10)) }
        button("Громкость −") { control("volume_delta", JSONObject().put("delta", -5)) }
        button("Громкость +") { control("volume_delta", JSONObject().put("delta", 5)) }
        button("Обновить состояние плеера") { control("get_status") }
        status = TextView(this).apply {
            text = "Не подключено"; textSize = 14f; setTextColor(0xFFB9C9D9.toInt())
            setPadding(0, 20, 0, 20)
        }
        content.addView(status)
        scroll.addView(content)
        root.addView(scroll)
        setContentView(root)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == pickFile && resultCode == RESULT_OK) {
            selectedUri = data?.data
            status.text = selectedUri?.let { "Выбран файл: ${displayName(it)}" } ?: "Файл не выбран"
        }
    }

    private fun displayName(uri: Uri): String {
        var name = "media.bin"
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) name = it.getString(0) ?: name
        }
        return name
    }

    private fun transferAsync(uri: Uri) {
        val h = host.text.toString().trim()
        val p = port.text.toString().trim()
        val c = code.text.toString().trim().uppercase()
        prefs.edit().putString("host", h).putString("port", p).apply()
        status.text = "Подключаемся и проверяем файл…"
        Thread {
            try {
                val name = displayName(uri)
                val size = contentResolver.openFileDescriptor(uri, "r")?.use { it.statSize } ?: -1L
                if (size <= 0 || size > MAX_BYTES) error("Файл пустой или больше 512 МиБ.")
                val digest = MessageDigest.getInstance("SHA-256")
                contentResolver.openInputStream(uri).use { input ->
                    requireNotNull(input)
                    val buffer = ByteArray(CHUNK)
                    while (true) { val n = input.read(buffer); if (n < 0) break; digest.update(buffer, 0, n) }
                }
                val hash = digest.digest().joinToString("") { "%02x".format(it) }
                val ws = connect(h, p, c)
                try {
                    val ready = request(ws, "begin_media", JSONObject()
                        .put("file_name", name).put("mime_type", contentResolver.getType(uri) ?: "application/octet-stream")
                        .put("size", size).put("sha256", hash))
                    if (ready.optString("type") != "media_ready") error("ПК не подготовил приём файла.")
                    contentResolver.openInputStream(uri).use { input ->
                        requireNotNull(input)
                        val buffer = ByteArray(CHUNK)
                        while (true) {
                            val n = input.read(buffer)
                            if (n < 0) break
                            if (!ws.send(ByteString.of(*buffer.copyOf(n)))) error("Соединение прервано во время передачи.")
                            while (ws.queueSize() > 4L * CHUNK) Thread.sleep(30)
                        }
                    }
                    val done = request(ws, "finish_media", JSONObject())
                    if (done.optString("type") != "media_complete" || done.optJSONObject("payload")?.optString("playback") != "playing")
                        error("Файл принят, но воспроизведение не запустилось.")
                    show("Готово: $name")
                } finally { ws.close(1000, "done") }
            } catch (e: Exception) { show("Ошибка: ${e.message ?: e.javaClass.simpleName}") }
        }.start()
    }

    private fun control(type: String, payload: JSONObject = JSONObject()) {
        val h = host.text.toString().trim(); val p = port.text.toString().trim(); val c = code.text.toString().trim().uppercase()
        Thread {
            try {
                val ws = connect(h, p, c)
                try {
                    val response = request(ws, type, payload)
                    if (type == "get_status") show(response.optJSONObject("payload")?.toString(2) ?: "Нет состояния")
                    else if (response.optString("type") != "command_result") error("ПК не подтвердил команду.")
                    else show("Команда выполнена: $type")
                } finally { ws.close(1000, "done") }
            } catch (e: Exception) { show("Ошибка: ${e.message ?: e.javaClass.simpleName}") }
        }.start()
    }

    private fun connect(address: String, portText: String, pairingCode: String): WebSocket {
        require(address.isNotBlank()) { "Укажи IP компьютера." }
        val token = prefs.getString("device_token", "") ?: ""
        val socket = SocketSession()
        val ws = client.newWebSocket(Request.Builder().url("ws://$address:${portText.toInt()}/ws").build(), socket)
        socket.awaitOpen()
        val hello = socket.request(ws, "hello", JSONObject())
        if (hello.optString("type") != "hello_ack") error("AERIS Desktop не ответил на hello.")
        val pairPayload = if (token.isNotBlank()) JSONObject().put("token", token) else JSONObject().put("code", pairingCode)
        var paired = socket.request(ws, "pair", pairPayload)
        if (paired.optJSONObject("payload")?.optBoolean("paired") != true && token.isNotBlank()) {
            paired = socket.request(ws, "pair", JSONObject().put("code", pairingCode))
        }
        if (paired.optJSONObject("payload")?.optBoolean("paired") != true) error("Не удалось выполнить сопряжение. Проверь код на ПК.")
        val newToken = paired.optJSONObject("payload")?.optString("token")
        if (!newToken.isNullOrBlank()) prefs.edit().putString("device_token", newToken).apply()
        socket.session = ws
        socket.queue = socket.messages
        return ws
    }

    private fun request(ws: WebSocket, type: String, payload: JSONObject): JSONObject {
        val body = JSONObject().put("protocol", "aeris").put("version", 1).put("type", type)
            .put("id", System.nanoTime().toString()).put("payload", payload)
        if (!ws.send(body.toString())) error("Не удалось отправить команду.")
        val response = activeSession?.takeMessage() ?: error("Нет активной WebSocket-сессии")
        if (response.optString("type") == "error") error(response.optJSONObject("payload")?.optString("message") ?: "Ошибка AERIS")
        return response
    }

    private var activeSession: SocketSession? = null

    private inner class SocketSession : WebSocketListener() {
        val messages = LinkedBlockingQueue<String>()
        var session: WebSocket? = null
        var queue: LinkedBlockingQueue<String>? = null
        private val opened = java.util.concurrent.CountDownLatch(1)
        private var failure: String? = null
        override fun onOpen(webSocket: WebSocket, response: Response) { session = webSocket; activeSession = this; opened.countDown() }
        override fun onMessage(webSocket: WebSocket, text: String) { messages.offer(text) }
        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) { failure = t.message; opened.countDown(); messages.offer(JSONObject().put("type", "transport_error").put("message", failure).toString()) }
        fun awaitOpen() {
            if (!opened.await(10, TimeUnit.SECONDS)) error("Тайм-аут подключения.")
            if (failure != null) error(failure ?: "Ошибка WebSocket")
        }
        fun request(ws: WebSocket, type: String, payload: JSONObject): JSONObject {
            val body = JSONObject().put("protocol", "aeris").put("version", 1).put("type", type)
                .put("id", System.nanoTime().toString()).put("payload", payload)
            if (!ws.send(body.toString())) error("Не удалось отправить запрос.")
            return takeMessage()
        }
        fun takeMessage(): JSONObject {
            val raw = messages.poll(15, TimeUnit.SECONDS) ?: error("ПК не ответил за 15 секунд.")
            val result = JSONObject(raw)
            if (result.optString("type") == "transport_error") error(result.optString("message"))
            return result
        }
    }

    private fun show(message: String) { runOnUiThread { status.text = message } }

    override fun onDestroy() {
        client.dispatcher.cancelAll()
        super.onDestroy()
    }

    companion object {
        private const val CHUNK = 1024 * 1024
        private const val MAX_BYTES = 512L * 1024L * 1024L
    }
}

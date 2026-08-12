package app.neko.finance.securevault

import android.app.Activity
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
internal class SecretKeyArgs {
    lateinit var service: String
    lateinit var username: String
}

@InvokeArg
internal class StoreArgs {
    lateinit var service: String
    lateinit var username: String
    lateinit var secret: String
}

private const val PREFS_FILE_NAME = "neko_finance_secure_vault"
private const val KEY_SEPARATOR = ' '

/**
 * O braço Android do cofre de segredos (ADR-0014, cláusula 2): três comandos — store/load/delete
 * — sobre um único arquivo `EncryptedSharedPreferences`, cifrado por uma chave AES-256 que o
 * Android Keystore gera e nunca exporta em claro. `MasterKey`/`EncryptedSharedPreferences` são a
 * rota recomendada pelo Android Jetpack Security para segredo pequeno em repouso — a mesma
 * garantia que o Keychain (macOS), o Credential Manager (Windows) e o Secret Service (Linux) já
 * davam do lado desktop via `keyring`.
 *
 * Sem API JavaScript: só o Rust deste app (`secret_vault::AndroidVault`, via
 * `PluginHandle::run_mobile_plugin`) chama estes comandos.
 */
@TauriPlugin
class SecureVaultPlugin(private val activity: Activity) : Plugin(activity) {

    private fun preferences(): SharedPreferences {
        val context = activity.applicationContext
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        return EncryptedSharedPreferences.create(
            context,
            PREFS_FILE_NAME,
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
        )
    }

    // O separador é um espaço: serviço e usuário aqui são sempre constantes do próprio app
    // (nunca entrada livre), então a chave composta nunca ambiguiza um par diferente.
    private fun entryKey(service: String, username: String) = service + KEY_SEPARATOR + username

    @Command
    fun store(invoke: Invoke) {
        val args = invoke.parseArgs(StoreArgs::class.java)
        // `commit()`, não `apply()`: a chamada Rust é síncrona e espera que um `load` logo em
        // seguida já enxergue o valor gravado — `apply()` devolveria antes de o disco confirmar.
        preferences()
            .edit()
            .putString(entryKey(args.service, args.username), args.secret)
            .commit()
        invoke.resolve(JSObject())
    }

    @Command
    fun load(invoke: Invoke) {
        val args = invoke.parseArgs(SecretKeyArgs::class.java)
        val secret = preferences().getString(entryKey(args.service, args.username), null)
        val ret = JSObject()
        ret.put("secret", secret)
        invoke.resolve(ret)
    }

    @Command
    fun delete(invoke: Invoke) {
        val args = invoke.parseArgs(SecretKeyArgs::class.java)
        preferences()
            .edit()
            .remove(entryKey(args.service, args.username))
            .commit()
        invoke.resolve(JSObject())
    }
}

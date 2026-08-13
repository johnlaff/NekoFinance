package app.neko.finance

import android.content.Context
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  // O `tao` 0.35.x nunca migrou de `ndk-glue` para `ndk-context` (tauri-apps/tao#1220), então
  // nada na árvore de dependências do Tauri popula `ndk_context::android_context()` — sem esta
  // ponte, o cofre de segredos Android (`secret_vault.rs`) sonda um contexto que nunca fica
  // pronto e nenhuma chave persiste. Corrigido a montante em tauri-apps/tao#1266, publicado no
  // tao 0.36.0 (2026-07-29); remover esta declaração, a chamada abaixo e a função JNI
  // correspondente em `secret_vault.rs` quando o Tauri publicar uma versão que carregue
  // tao >= 0.36 (verificado 2026-08: `tauri` 2.11.5 ainda resolve tao 0.35.3).
  private external fun initNdkContext(context: Context)

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Application Context, nunca `this` (a Activity): o contexto precisa sobreviver a uma
    // recriação de Activity (mudança de configuração) sem apontar para um objeto já destruído.
    initNdkContext(this.applicationContext)
  }

  // O WebView do Android ignora a preferência de tamanho de fonte do sistema por padrão —
  // textZoom fica travado em 100% mesmo com Configurações > Acessibilidade > Tamanho da
  // fonte pedindo texto maior. `resources.configuration.fontScale` já chega como o fator
  // que o sistema aplica nativamente (1.0 = padrão); textZoom espera o mesmo fator em
  // percentual. `fontScale` não está entre os `configChanges` do manifesto, então uma
  // mudança em runtime recria a Activity e chama este método de novo com o valor atual.
  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webView.settings.textZoom = (resources.configuration.fontScale * 100).toInt()
  }
}

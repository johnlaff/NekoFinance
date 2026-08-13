package app.neko.finance

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
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

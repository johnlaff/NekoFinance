import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "app.neko.finance.securevault"
    compileSdk = 36

    defaultConfig {
        // O mesmo piso do app (`gen/android/app/build.gradle.kts`) — o Keystore do sistema
        // (API 23+) já cobre todo o alcance real do app.
        minSdk = 24

        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.12.0")
    // O Keystore do sistema por trás de duas chamadas — `MasterKey` gera/abre a chave AES no
    // Keystore, `EncryptedSharedPreferences` cifra o valor com ela. É a rota recomendada pelo
    // próprio Android (Jetpack Security) para segredo pequeno em repouso, e evita reimplementar
    // o par KeyGenParameterSpec/Cipher à mão sobre a API de Keystore crua.
    implementation("androidx.security:security-crypto:1.1.0")
    implementation(project(":tauri-android"))
}

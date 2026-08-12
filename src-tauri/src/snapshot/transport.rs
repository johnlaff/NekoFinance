//! Transporte do snapshot pelo `appDataFolder` do Drive, atrás da borda HTTP existente
//! (`crate::http`). `appDataFolder` é o espaço reservado do PRÓPRIO app — invisível ao dono e aos
//! demais apps, coerente com o escopo estreito `drive.appdata` (oauth::pkce).

use super::manifest::SnapshotManifest;
use crate::google_sheets::google_error;
use crate::oauth::token_store::StoredToken;
use serde::Deserialize;

/// Nomes fixos dos dois arquivos que o app mantém no `appDataFolder` — um snapshot e um manifest
/// não competem entre si por nome (a pasta é exclusiva do app, mas os dois papéis são distintos).
const MANIFEST_FILE_NAME: &str = "neko-manifest.json";
const SNAPSHOT_FILE_NAME: &str = "neko-snapshot.db";

/// A base real da API do Drive. Injetável no cliente para os testes apontarem a um servidor
/// mockado — nenhum teste do transporte toca a rede.
pub fn production_base_url() -> &'static str {
    "https://www.googleapis.com"
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DriveFileList {
    #[serde(default)]
    files: Vec<DriveFile>,
}

/// Monta o corpo `multipart/related` (RFC 2387) exigido pelo upload multipart do Drive: uma parte
/// JSON de metadados + uma parte de mídia. Função PURA — o boundary é um UUID gerado por quem
/// chama, então nunca colide com o conteúdo das partes.
fn build_multipart_related(
    boundary: &str,
    metadata_json: &str,
    media_content_type: &str,
    media: &[u8],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(media.len() + metadata_json.len() + 256);
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(metadata_json.as_bytes());
    body.extend_from_slice(
        format!("\r\n--{boundary}\r\nContent-Type: {media_content_type}\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(media);
    body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());
    body
}

pub struct DriveSnapshotClient {
    token: StoredToken,
    base_url: String,
}

impl DriveSnapshotClient {
    pub fn new(token: StoredToken, base_url: impl Into<String>) -> Self {
        Self {
            token,
            base_url: base_url.into(),
        }
    }

    /// Acha o `fileId` de um nome no `appDataFolder`, ou `None` se ele ainda não existe — a
    /// pasta é exclusiva do app (escopo `drive.appdata`), então o nome já é a identidade.
    async fn find_file_id(&self, name: &str) -> Result<Option<String>, String> {
        let url = format!("{}/drive/v3/files", self.base_url);
        let q = format!("name = '{name}' and trashed = false");
        let resp = crate::http::send_with_retry(
            crate::http::client()
                .get(&url)
                .query(&[
                    ("spaces", "appDataFolder"),
                    ("q", q.as_str()),
                    ("fields", "files(id)"),
                ])
                .bearer_auth(&self.token.access_token),
        )
        .await
        .map_err(|e| format!("drive files.list error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(google_error("Drive API", status, &body));
        }

        let list: DriveFileList = resp
            .json()
            .await
            .map_err(|e| format!("drive files.list parse: {e}"))?;
        Ok(list.files.into_iter().next().map(|f| f.id))
    }

    /// Baixa e valida o manifest publicado por qualquer aparelho — `None` quando nenhum snapshot
    /// foi publicado ainda (primeira subida, ver `snapshot::lease`).
    pub async fn fetch_manifest(&self) -> Result<Option<SnapshotManifest>, String> {
        let Some(file_id) = self.find_file_id(MANIFEST_FILE_NAME).await? else {
            return Ok(None);
        };

        let url = format!("{}/drive/v3/files/{file_id}", self.base_url);
        let resp = crate::http::send_with_retry(
            crate::http::client()
                .get(&url)
                .query(&[("alt", "media")])
                .bearer_auth(&self.token.access_token),
        )
        .await
        .map_err(|e| format!("drive files.get error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(google_error("Drive API", status, &body));
        }

        let manifest: SnapshotManifest = resp
            .json()
            .await
            .map_err(|e| format!("manifest parse: {e}"))?;
        Ok(Some(manifest))
    }

    /// Publica o snapshot (`db_bytes`, um arquivo SQLite íntegro produzido por `VACUUM INTO`) e o
    /// manifest que o acompanha. Cria os dois arquivos na primeira subida; nas seguintes,
    /// atualiza os MESMOS `fileId` (nunca duplica arquivo no `appDataFolder`).
    pub async fn upload_snapshot(
        &self,
        db_bytes: &[u8],
        manifest: &SnapshotManifest,
    ) -> Result<(), String> {
        let existing_snapshot_id = self.find_file_id(SNAPSHOT_FILE_NAME).await?;
        self.put_file(
            SNAPSHOT_FILE_NAME,
            existing_snapshot_id,
            "application/octet-stream",
            db_bytes,
        )
        .await?;

        let manifest_json =
            serde_json::to_vec(manifest).map_err(|e| format!("serializar manifest: {e}"))?;
        let existing_manifest_id = self.find_file_id(MANIFEST_FILE_NAME).await?;
        self.put_file(
            MANIFEST_FILE_NAME,
            existing_manifest_id,
            "application/json",
            &manifest_json,
        )
        .await?;

        Ok(())
    }

    async fn put_file(
        &self,
        name: &str,
        existing_id: Option<String>,
        media_content_type: &str,
        media: &[u8],
    ) -> Result<(), String> {
        let boundary = format!("neko-{}", uuid::Uuid::new_v4());
        // Update (PATCH) não pode repetir `parents` — o Drive rejeita realocar um arquivo já na
        // pasta. Só a criação (POST) declara `appDataFolder` como pai.
        let metadata = if existing_id.is_some() {
            serde_json::json!({ "name": name })
        } else {
            serde_json::json!({ "name": name, "parents": ["appDataFolder"] })
        };
        let metadata_json =
            serde_json::to_string(&metadata).map_err(|e| format!("metadata: {e}"))?;
        let body = build_multipart_related(&boundary, &metadata_json, media_content_type, media);
        let content_type = format!("multipart/related; boundary={boundary}");

        let req = match &existing_id {
            Some(id) => {
                crate::http::client().patch(format!("{}/upload/drive/v3/files/{id}", self.base_url))
            }
            None => crate::http::client().post(format!("{}/upload/drive/v3/files", self.base_url)),
        }
        .query(&[("uploadType", "multipart")])
        .bearer_auth(&self.token.access_token)
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .body(body);

        let resp = crate::http::send_with_retry(req)
            .await
            .map_err(|e| format!("drive upload request: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(google_error("Drive upload", status, &text));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::manifest::SnapshotManifest;
    use super::*;
    use crate::oauth::token_store::StoredToken;

    fn token() -> StoredToken {
        StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 9_999_999_999,
            scope: "".into(),
        }
    }

    fn manifest() -> SnapshotManifest {
        SnapshotManifest {
            device_id: "device-a".into(),
            sequence: 1,
            created_at: "2026-08-11T12:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 42,
        }
    }

    #[test]
    fn multipart_related_body_carries_both_parts_with_the_right_content_types() {
        let body =
            build_multipart_related("B", "{\"name\":\"x\"}", "application/octet-stream", b"abc");
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with(
            "--B\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{\"name\":\"x\"}"
        ));
        assert!(text.contains("\r\n--B\r\nContent-Type: application/octet-stream\r\n\r\nabc"));
        assert!(text.ends_with("\r\n--B--"));
    }

    #[tokio::test]
    async fn fetch_manifest_is_none_when_no_manifest_file_exists_yet() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("spaces".into(), "appDataFolder".into()),
                mockito::Matcher::UrlEncoded(
                    "q".into(),
                    "name = 'neko-manifest.json' and trashed = false".into(),
                ),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;

        let client = DriveSnapshotClient::new(token(), server.url());
        let found = client.fetch_manifest().await.expect("fetch_manifest");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn fetch_manifest_downloads_and_parses_when_present() {
        let mut server = mockito::Server::new_async().await;
        let _list = server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"files": [{"id": "manifest-123", "name": "neko-manifest.json"}]}"#)
            .create_async()
            .await;
        let manifest_json = serde_json::to_string(&manifest()).unwrap();
        let _get = server
            .mock("GET", "/drive/v3/files/manifest-123")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(manifest_json)
            .create_async()
            .await;

        let client = DriveSnapshotClient::new(token(), server.url());
        let found = client.fetch_manifest().await.expect("fetch_manifest");
        assert_eq!(found, Some(manifest()));
    }

    #[tokio::test]
    async fn fetch_manifest_surfaces_drive_error_body() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "insufficient scope"}}"#)
            .create_async()
            .await;

        let client = DriveSnapshotClient::new(token(), server.url());
        let err = client.fetch_manifest().await.unwrap_err();
        assert!(err.contains("insufficient scope"), "erro devolvido: {err}");
    }

    #[tokio::test]
    async fn upload_snapshot_creates_both_files_when_neither_exists() {
        let mut server = mockito::Server::new_async().await;
        // Nenhum arquivo existente ainda: toda busca por nome devolve lista vazia.
        let _list = server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let create_snapshot = server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "uploadType".into(),
                "multipart".into(),
            ))
            .match_body(mockito::Matcher::Regex("neko-snapshot.db".into()))
            .with_status(200)
            .with_body(r#"{"id": "snap-1"}"#)
            .expect(1)
            .create_async()
            .await;
        let create_manifest = server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "uploadType".into(),
                "multipart".into(),
            ))
            .match_body(mockito::Matcher::Regex("neko-manifest.json".into()))
            .with_status(200)
            .with_body(r#"{"id": "man-1"}"#)
            .expect(1)
            .create_async()
            .await;

        let client = DriveSnapshotClient::new(token(), server.url());
        client
            .upload_snapshot(b"SQLite format 3\0fake-db-bytes", &manifest())
            .await
            .expect("upload_snapshot");

        create_snapshot.assert_async().await;
        create_manifest.assert_async().await;
    }

    #[tokio::test]
    async fn upload_snapshot_updates_existing_files_by_id_instead_of_creating() {
        let mut server = mockito::Server::new_async().await;
        let _list_snapshot = server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "snap-existing", "name": "neko-snapshot.db"}]}"#)
            .create_async()
            .await;
        let _list_manifest = server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-existing", "name": "neko-manifest.json"}]}"#)
            .create_async()
            .await;
        let update_snapshot = server
            .mock("PATCH", "/upload/drive/v3/files/snap-existing")
            .match_query(mockito::Matcher::UrlEncoded(
                "uploadType".into(),
                "multipart".into(),
            ))
            .with_status(200)
            .with_body(r#"{"id": "snap-existing"}"#)
            .expect(1)
            .create_async()
            .await;
        let update_manifest = server
            .mock("PATCH", "/upload/drive/v3/files/man-existing")
            .match_query(mockito::Matcher::UrlEncoded(
                "uploadType".into(),
                "multipart".into(),
            ))
            .with_status(200)
            .with_body(r#"{"id": "man-existing"}"#)
            .expect(1)
            .create_async()
            .await;

        let client = DriveSnapshotClient::new(token(), server.url());
        client
            .upload_snapshot(b"SQLite format 3\0fake-db-bytes", &manifest())
            .await
            .expect("upload_snapshot");

        update_snapshot.assert_async().await;
        update_manifest.assert_async().await;
    }

    #[tokio::test]
    async fn upload_snapshot_fails_closed_on_transport_error_without_panicking() {
        let mut server = mockito::Server::new_async().await;
        let _list = server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let _create = server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(500)
            .with_body(r#"{"error": {"message": "backend hiccup"}}"#)
            .create_async()
            .await;

        let client = DriveSnapshotClient::new(token(), server.url());
        let err = client
            .upload_snapshot(b"bytes", &manifest())
            .await
            .unwrap_err();
        assert!(err.contains("backend hiccup"), "erro devolvido: {err}");
    }
}

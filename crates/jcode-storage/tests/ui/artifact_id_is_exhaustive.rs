use jcode_storage::ArtifactId;

fn exhaustive(id: ArtifactId) {
    match id {
        ArtifactId::ClaudeOauth
        | ArtifactId::OpenAiOauth
        | ArtifactId::GoogleCredentials
        | ArtifactId::GoogleOauth
        | ArtifactId::KimiCredentials
        | ArtifactId::KimiDeviceId
        | ArtifactId::GrokDirectCredentials
        | ArtifactId::ProviderEnvFile
        | ArtifactId::ExternalClaudeCredentials
        | ArtifactId::ExternalOpenCodeCredentials
        | ArtifactId::ConfigToml
        | ArtifactId::ProviderActivity
        | ArtifactId::AuthRefreshState
        | ArtifactId::AmbientState
        | ArtifactId::AmbientQueue
        | ArtifactId::AmbientTranscripts
        | ArtifactId::PendingSoftInterrupt
        | ArtifactId::SessionInboxItem
        | ArtifactId::LegacySessionRoot
        | ArtifactId::SwarmState
        | ArtifactId::SwarmControlLog
        | ArtifactId::ServerBeacon
        | ArtifactId::DeliveryCampaign
        | ArtifactId::BackgroundTaskStatus
        | ArtifactId::DebugControl
        | ArtifactId::LastFocusedClientSession
        | ArtifactId::LastSeenChangelog
        | ArtifactId::ModelCatalogCache
        | ArtifactId::ModelCache
        | ArtifactId::EndpointCache
        | ArtifactId::SessionSearchCache
        | ArtifactId::SessionPickerListCache
        | ArtifactId::MermaidRender
        | ArtifactId::LatexRender
        | ArtifactId::ClientInputScratch
        | ArtifactId::VisualDebug
        | ArtifactId::LiveTestCoverage
        | ArtifactId::LiveTestEvents
        | ArtifactId::SessionLock
        | ArtifactId::ConfigLock
        | ArtifactId::PidMarkerLock
        | ArtifactId::GrokDirectCredentialsLock
        | ArtifactId::DaemonLock
        | ArtifactId::DaemonSocket => {}
    }
}

fn main() {
    exhaustive(ArtifactId::ConfigToml);
}

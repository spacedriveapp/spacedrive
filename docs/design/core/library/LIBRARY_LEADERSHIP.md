<!--CREATED: 2025-06-29-->
Spacedrive Core v2: Sync Leadership & Key Exchange Protocol
Date: June 27, 2025
Status: Proposed Design
Author: Gemini

1. Overview
   This document specifies the design for two critical components of Spacedrive Core v2's multi-device synchronization system: a user-driven protocol for managing sync leadership and a secure protocol for sharing library access between paired devices.

This design refines the concepts in SYNC_DESIGN.md by replacing complex, automatic leader election with a pragmatic, user-controlled Leader Promotion Model. This approach prioritizes stability and data integrity, acknowledging the intended architecture where at least one device per library (e.g., a self-hosted server) is "always-on".

Furthermore, it formalizes the Secure Library Key Exchange Protocol, detailing how a device can safely receive the necessary cryptographic keys to join and sync a library, leveraging the trusted channel established during initial device pairing.

2. Part 1: Pragmatic Leader Promotion Model
   This model is founded on the principle that leadership of a library's sync log is a deliberate administrative role, not a dynamically shifting one. Changes in leadership are explicit, observable, and controlled by the user.

2.1. Initial Leader Selection

The first leader is designated when sync is enabled for a library.

Trigger: A user initiates sync for a library between two or more devices via the SyncSetupJob.

Mechanism: The UI will prompt the user to explicitly select which device will act as the leader for that library. This choice is final until another promotion is manually initiated.

Storage: The device_id of the chosen leader, along with an initial epoch number (e.g., 1), is stored in the library's library.json configuration file. This file is then distributed to all participating devices as the unambiguous source of truth for leadership.

2.2. Leadership Handover: The promote-leader Command

A leadership change is an administrative task triggered via the CLI. This prevents unintended leadership changes due to transient network issues.

2.2.1. CLI Command

A new command will be added to the spacedrive CLI:

spacedrive library promote-leader --library-id <UUID> --new-leader-device-id <UUID> [--force]

--library-id: The UUID of the library whose leader is being changed.

--new-leader-device-id: The UUID of the follower device being promoted.

--force: An optional flag for disaster recovery. It allows promoting a new leader even if the current leader is offline. This action requires explicit user confirmation due to the risk of creating a split-brain scenario if the old leader later comes back online unaware of the change.

2.2.2. The LeaderPromotionJob

Executing the command dispatches a LeaderPromotionJob. This ensures the complex, multi-step process is reliable, resumable, and provides clear progress feedback to the user, consistent with Spacedrive's job-based architecture.

Job Definition (src/sync/jobs/leader_promotion.rs):

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct LeaderPromotionJob {
pub library_id: Uuid,
pub new_leader_id: Uuid,
pub old_leader_id: Uuid,
pub force: bool,

    // Internal state for resumability
    #[serde(skip)]
    state: PromotionState,

}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum PromotionState {
Pending,
PreFlightChecks,
PausingSync,
ExportingLog,
TransferringLog,
ImportingLog,
ConfirmingHandover,
ResumingSync,
Complete,
Failed,
}

// ... Job and JobHandler implementations ...

2.2.3. Promotion Workflow State Machine

The LeaderPromotionJob executes the following state machine:

Pre-flight Checks:

Verify the new leader device is online and fully synced with the current leader's log. A promotion cannot proceed if the candidate is behind.

If --force is not used, verify the current leader is also online. If not, the job fails with a message instructing the user to use --force for disaster recovery.

Pause Library Sync (Quiescence):

The current leader broadcasts a PauseSync message to all followers for the library.

Followers receive this message, stop processing local changes for that library, and enter a "paused" state, awaiting the promotion to complete.

Sync Log Transfer:

The current leader serializes and compresses its entire sync_log for the specified library.

It initiates a standard, robust file transfer to send the log export to the new leader device, reusing the battle-tested protocol demonstrated in test_core_file_transfer.rs.

Verification & Import:

The new leader receives and verifies the integrity of the log file.

Upon successful verification, it replaces its local (follower) copy of the sync log with the authoritative version from the old leader.

The Handover:

Epoch Increment: The new leader increments the library's epoch number by one.

Role Update: The new leader updates its own sync_leadership status to Leader for the library and new epoch.

Broadcast Confirmation: The new leader broadcasts a NewLeaderConfirmed message, which includes the new_leader_id and the new_epoch. This message is cryptographically signed by the new leader.

Demotion & Confirmation: The old leader and all followers receive the NewLeaderConfirmed message. They verify the signature, update their library.json to point to the new leader, update the epoch, and (in the case of the old leader) demote their role to Follower.

Resume Sync:

The new leader broadcasts a ResumeSync message.

All followers, now aware of the new leader and epoch, resume normal sync operations, directing all future communication to the new leader. Any stray messages from the old leader are rejected due to the outdated epoch.

2.2.4. Network Messages

This model requires only three new simple messages within the DeviceMessage enum.

// src/services/networking/core/behavior.rs
pub enum DeviceMessage {
// ... existing messages ...

    // Leader Promotion Messages
    PauseSync { library_id: Uuid },
    ResumeSync { library_id: Uuid },
    NewLeaderConfirmed {
        library_id: Uuid,
        new_leader_id: Uuid,
        new_epoch: u64,
        // The message should be signed to prove the new leader's identity
        signature: Vec<u8>,
    },

}

3. Secure Library Key Exchange Protocol
   This protocol enables a new device to join an existing library by securely obtaining the library_key required to decrypt its contents. The entire exchange is protected by the session keys generated during the initial, trusted device pairing process.

3.1. Trigger

The protocol is initiated by a user action after two devices have successfully paired. For example, a "Share Library" button in the UI on Device A would show a list of its paired devices, including Device B.

3.2. Protocol Messages

The exchange uses a new set of messages within the DeviceMessage enum.

// src/services/networking/core/behavior.rs
pub enum DeviceMessage {
// ... existing messages ...

    // Library Key Exchange Messages
    ShareLibraryRequest {
        library_id: Uuid,
        library_name: String,
    },
    ShareLibraryResponse {
        library_id: Uuid,
        accepted: bool,
    },
    LibraryKeyShare {
        library_id: Uuid,
        encrypted_library_key: Vec<u8>,
        nonce: [u8; 12], // For ChaCha20-Poly1305
    },
    ShareComplete {
        library_id: Uuid,
        success: bool,
    },

}

3.3. Key Exchange Workflow

Let's assume Device A (Owner) has the library and has just paired with Device B (Joiner).

Initiation (Owner):

The user on Device A selects a library and chooses to share it with the newly paired Device B.

Device A sends a ShareLibraryRequest to B.

User Consent (Joiner):

Device B receives the request. Its UI prompts the user: "Device A (MacBook Pro) wants to share the library 'Family Photos' with you. Allow?"

If the user on B accepts, B sends a ShareLibraryResponse { accepted: true } back to A.

Secure Key Transmission (Owner):

Device A receives the acceptance.

It retrieves the plaintext library_key from its secure OS keyring via the LibraryKeyManager.

It retrieves the session keys established during pairing for Device B from its DeviceRegistry.

It encrypts the library_key using the session send_key. An AEAD cipher like ChaCha20-Poly1305 is used to ensure confidentiality and authenticity.

Device A sends the LibraryKeyShare message, containing the encrypted_library_key and nonce, to B.

Receipt and Storage (Joiner):

Device B receives the LibraryKeyShare.

It uses its corresponding session receive_key to decrypt the payload.

Upon successful decryption, it stores the recovered plaintext library_key in its own secure OS keyring via its LibraryKeyManager, associating it with the received library_id.

Confirmation and Sync:

Device B sends a ShareComplete { success: true } message to A.

The key exchange is complete. Device B now has the necessary key to decrypt the library's database and can dispatch an InitialSyncJob to begin syncing the library as a follower.

4. Conclusion
   This design establishes a secure, robust, and user-centric foundation for multi-device collaboration in Spacedrive.

The Pragmatic Leader Promotion Model replaces complex automatic elections with a deliberate, job-based administrative process. This enhances stability, prevents data corruption, and aligns with the intended use case of an always-on device acting as a stable leader.

The Secure Library Key Exchange Protocol provides a simple yet cryptographically secure method for granting new devices access to a library, building upon the trust established during the initial device pairing.

By integrating these protocols into the existing job-based and event-driven architecture, Spacedrive can offer powerful multi-device sync features without sacrificing user control or data integrity.

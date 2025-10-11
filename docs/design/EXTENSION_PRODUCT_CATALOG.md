# Spacedrive Extensions: Own Your Data, Supercharge Your Workflow

**A new ecosystem of applications built on a foundation of privacy, control, and intelligence.**

Spacedrive is more than a file manager; it's a Virtual Distributed File System (VDFS) that unifies your data across every device and cloud into a single, private library. Extensions are powerful applications that run directly on this platform, inheriting its powerful infrastructure for storage, sync, search, and AI.

This is a new paradigm for software:
-   **Local-First & Private:** Your data lives on your devices. No vendor lock-in, no surrendering your privacy for convenience.
-   **Data That Lasts:** Data ingested by an extension (like receipts, passwords, or research) remains in your VDFS forever, even if you stop using the extension.
-   **Powerful Synergy:** Extensions don't live in silos. They communicate and build upon each other, creating workflows that were never before possible.
-   **AI-Native:** Agents and AI models can operate across your entire data library, providing insights and automation that understand the complete context of your life.

---

## The Spacedrive Collection: Paid Extensions

Our suite of paid extensions provides powerful, specialized functionality. They are designed to be best-in-class applications, supercharged by the Spacedrive platform.

### **Personal Bundle**

Get the essential suite of personal productivity tools at a significant discount.

-   **Includes:** Chronicle, Cipher, and Ledger
-   **Pricing:**
    -   **$20/month**
    -   **$400 Lifetime** (Limited availability, offered through Q1 2026)

---

### 1. Chronicle: Your Personal AI Research Assistant

*Turn digital chaos into a structured knowledge base.*

Chronicle ingests everything—articles, videos, PDFs, voice notes—and makes it searchable, understandable, and interconnected. It's a second brain that works for you, built on a foundation of your own data.

-   **Use Cases:** Academic research, market analysis, personal knowledge management, content creation.
-   **Key Features:**
    -   AI-powered summarization and concept extraction.
    -   Visual knowledge graph to see connections between ideas.
    -   Query your entire library with local (Ollama) or cloud AI models.
    -   Automatically captures and organizes sources.
-   **Technical Snapshot:** Defines models for `DocumentAnalysis`, `ResearchProject`, `Concept`, and `Note`.
-   **Pricing:** $10/month (for cloud AI features, core is open source)

### 2. Ledger: Automate Your Finances, Master Your Spending

*Transform your digital receipts and bank statements from static files into a dynamic financial dashboard.*

Ledger finds financial documents across your devices and turns them into structured data. Track spending, prepare for taxes, and gain true insight into your financial health, all with complete privacy.

-   **Use Cases:** Expense tracking, tax preparation, budgeting, small business accounting.
-   **Key Features:**
    -   Automatic receipt scanning and OCR for key details (merchant, amount, tax).
    -   Intelligent expense categorization.
    -   Budget creation and tracking with alerts.
    -   One-click tax document generation.
-   **Technical Snapshot:** Defines models for `ReceiptAnalysis`, `Budget`, and `TaxDocument`.
-   **Pricing:** $8/month

### 3. Atlas: The Dynamic, Local-First CRM

*Build the perfect system for managing your contacts, projects, and team knowledge.*

Atlas is a flexible, powerful database that adapts to your workflow. It's a CRM, a project manager, and a team wiki all in one, keeping your sensitive business data off third-party clouds and on your own devices.

-   **Use Cases:** Personal CRM, sales pipeline management, startup operations, client tracking.
-   **Key Features:**
    -   Flexible, user-definable data structures.
    -   Seamless integration with your emails and documents.
    -   Powerful semantic search across all related data.
    -   Designed for both individual prosumers and teams.
-   **Technical Snapshot:** Defines models for `Contact`, `Company`, `Deal`, and `Interaction`.
-   **Pricing:** $30/month

### 4. Cipher: Total Digital Security

*A next-generation password manager and encrypted vault for your most sensitive files.*

Cipher provides zero-knowledge security infrastructure for your entire digital life. It not only manages your credentials but also provides encryption services for other extensions, securing your data at its source.

-   **Use Cases:** Password management, secure file storage, digital identity protection.
-   **Key Features:**
    -   Zero-knowledge password and credential vault.
    -   Drag-and-drop file encryption.
    -   Password breach monitoring and security audits.
    -   Provides a core security layer for the entire Spacedrive ecosystem.
-   **Technical Snapshot:** Defines models for `Vault`, `Credential`, and `EncryptedFile`.
-   **Pricing:** $8/month

### 5. Studio: Your Creative Asset Hub

*For creators and teams who need to manage large libraries of photos, videos, and design files.*

Studio provides powerful, AI-driven tools for analyzing, versioning, and organizing your creative projects. It understands the content of your media, making it easy to find exactly what you need.

-   **Use Cases:** Video production, photography, graphic design, brand asset management.
-   **Key Features:**
    -   AI-powered video analysis (scene detection, transcription, speaker identification).
    -   Non-destructive versioning for all creative assets.
    -   Project-based organization.
    -   Advanced search by content (e.g., "find all videos with a sunset scene").
-   **Technical Snapshot:** Defines models for `VideoAnalysis`, `Project`, and `AssetVersion`.
-   **Pricing:** $15/month (projected)

---

## The Foundation: Open Source Archives

These free, open-source extensions are the bedrock of your personal data warehouse. They connect to your digital life, pulling your history into Spacedrive where it becomes permanent, searchable, and ready to be used by other extensions. They are the adoption drivers, demonstrating the power of a unified data library.

-   **Email Archive:** Ingests and indexes your email accounts.
    -   *Defines: `Email`, `EmailAccount`*
-   **Chrome History:** A complete, searchable archive of your browsing activity.
    -   *Defines: `BrowsingHistory`, `Bookmark`*
-   **Spotify Archive:** Your full listening history, playlists, and library.
    -   *Defines: `ListeningHistory`, `Playlist`*
-   **GPS Location History:** A private timeline of your physical world activity.
    -   *Defines: `LocationPoint`, `Visit`*
-   **Tweet Archive:** A local copy of your Twitter history.
    -   *Defines: `Tweet`, `TwitterAccount`*
-   **GitHub Repo Tracker:** Local mirror and analysis of repositories you care about.
    -   *Defines: `Repository`, `Contribution`*

---

## The Ecosystem Effect: Better Together

The true power of Spacedrive is unlocked when extensions work together. Because they share a common data layer, they can create seamless, automated workflows that were previously impossible.

-   **Email Archive → Ledger:** An email with a PDF receipt arrives. Ledger automatically sees it, scans the PDF, and adds it to your monthly expenses without you lifting a finger.
-   **Email Archive → Atlas:** You exchange emails with a new business contact. Atlas identifies the new person, creates a contact profile, and links the email thread to it automatically.
-   **Photos + GPS History:** Your phone's photo library is synced to Spacedrive. The GPS History extension correlates the photo timestamps with your location data, automatically organizing your photos into albums based on places you've visited.
-   **Chrome History → Chronicle:** While researching a topic, Chronicle observes your browsing and automatically pulls relevant articles and papers into your research project, linking them to the concepts you're exploring.

---

## For Developers: Build on Spacedrive

Ready to build the next generation of local-first software? The Spacedrive SDK gives you the infrastructure for sync, storage, search, and AI for free. Focus on your unique value proposition, not on reinventing the wheel.

By building on Spacedrive, you tap into a growing ecosystem and a user base that values data ownership and privacy.

**[→ Read the SDK Documentation](./sdk/sdk.md)**

<div align="center">

# ☠️ Automate Kill Task Manager

[![Tauri](https://img.shields.io/badge/Tauri-v2-FEC00F?style=for-the-badge&logo=tauri&logoColor=black)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Language-000000?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Vue.js](https://img.shields.io/badge/Vue.js-v3-4FC08D?style=for-the-badge&logo=vue.js&logoColor=white)](https://vuejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-Language-3178C6?style=for-the-badge&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Vite](https://img.shields.io/badge/Vite-Build_Tool-646CFF?style=for-the-badge&logo=vite&logoColor=white)](https://vitejs.dev/)

**Your personal enforcer for system resources.**  
_Auto-kill processes, manage blacklists, and monitor system health with absolute control._

[Report Bug](https://github.com/yourusername/automate-kill-task-manager/issues) · [Request Feature](https://github.com/yourusername/automate-kill-task-manager/issues)

</div>

---

## 📖 Introduction

**Automate Kill Task Manager** is a high-performance system utility built with the speed of Rust and the flexibility of Tauri. It gives you the power to define rules for how processes should behave on your Windows machine.

Tired of background tasks eating up your CPU? GPU miners sneaking in? Or just need to keep a specific set of apps closed? **This app handles it all automatically.**

## ✨ Key Features

### 🛡️ active-protection-system

- **Smart Blacklist**: Add processes to a "Kill List" and let the system hunt them down.
- **Auto-Kill Toggle**: Choose which blacklisted apps die immediately upon detection and which ones are just monitored.
- **Resource Triggers**:
  - **CPU Threshold**: Kill a process only if it exceeds $X$% CPU usage.
  - **GPU Threshold**: Nukes graphical intensive apps if they cross your defined GPU limit.

### 🖥️ Advanced System Monitor

- **Real-time Dashboard**: Live tracking of System CPU, Memory, and Disk Usage.
- **Process Watchlist**: Keep an eye on specific apps without killing them... yet.
- **Grouped View**: Like Task Manager, but cleaner. See aggregated stats for multi-process apps (e.g., Chrome, VS Code).

### 📝 Comprehensive Logging

- **Activity History**: Every kill is recorded. Know exactly **what** was killed, **when**, and **why**.
- **Kill-Only Mode**: Filter logs to see only the action, skipping the noise.

## 🛠️ Technology Stack

Built with cutting-edge modern web and system technologies:

- **Frontend**: [Vue 3](https://vuejs.org/) (Composition API) + TypeScript
- **Backend**: [Rust](https://www.rust-lang.org/) (for system-level API calls, process management)
- **Framework**: [Tauri v2](https://tauri.app/) (Lightweight, secure, and fast)
- **Styling**: Custom CSS with Dark Mode aesthetic

## 🚀 Getting Started

### Prerequisites

Ensure you have the following installed:

- **Node.js** (Latest LTS recommended)
- **Rust** (via `rustup`)
- **Visual Studio C++ Build Tools** (for Windows development)

### Installation

1.  **Clone the repository**

    ```bash
    git clone https://github.com/yourusername/automate-kill-task-manager.git
    cd automate-kill-task-manager
    ```

2.  **Install dependencies**

    ```bash
    npm install
    # or
    pnpm install
    # or
    yarn
    ```

3.  **Run Development Server**
    ```bash
    npm run tauri dev
    ```

### Building for Production

To create a standalone `.exe` installer:

```bash
npm run tauri build
```

The output will be available in `src-tauri/target/release/bundle/nsis/`.

## ⚠️ Admin Privileges

For full functionality (killing system processes or elevated tasks), **run the application as Administrator**.

- The app includes a built-in indicator to show if you currently have Admin rights.

---

<div align="center">

Made with ❤️ and 🦀 Rust

</div>

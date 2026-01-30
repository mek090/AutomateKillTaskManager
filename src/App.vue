<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";

// ============= Types =============

type ProcRow = {
  pid: number;
  name: string;
  cpu: number;
  memory_kb: number;
  gpu: number;
};

type DiskInfo = {
  name: string;
  mount_point: string;
  total_gb: number;
  used_gb: number;
  free_gb: number;
  usage_percent: number;
};

type SystemStats = {
  cpu_usage: number;
  memory_total_gb: number;
  memory_used_gb: number;
  memory_percent: number;
  disks: DiskInfo[];
};

type BlacklistEntry = {
  name: string;
  auto_kill: boolean;
  cpu_threshold: number;
  gpu_threshold: number;
  log_enabled: boolean;
  log_kills_only: boolean;
  created_at: string;
  kill_count: number;
};

type ActivityLog = {
  name: string;
  pid: number;
  cpu_usage: number;
  gpu_usage: number;
  detected_at: string;
  was_killed: boolean;
  reason: string;
};

type ProcessGroup = {
  name: string;
  process_count: number;
  pids: number[];
  total_cpu: number;
  total_memory_kb: number;
  total_gpu: number;
};

// ============= State =============

const activeTab = ref<"monitor" | "blacklist">("monitor");

// Monitor state
const watchlistInput = ref("");
const watchlist = ref<string[]>([]);
const rows = ref<ProcRow[]>([]);
const groupedRows = ref<ProcessGroup[]>([]);
const viewMode = ref<"grouped" | "detailed">("grouped");
const systemStats = ref<SystemStats | null>(null);
const isLoading = ref(false);
const statusMessage = ref("");

// Blacklist state
const blacklist = ref<BlacklistEntry[]>([]);
const activityLogs = ref<ActivityLog[]>([]);
const newBlacklistName = ref("");
const newAutoKill = ref(true);
const newCpuEnabled = ref(true);
const newCpuThreshold = ref(30); // Default 30%
const newGpuEnabled = ref(false);
const newGpuThreshold = ref(50); // Default 50%
const blacklistStatus = ref("");
const isAdmin = ref(false);

let timer: ReturnType<typeof setInterval> | undefined;

// ============= Sorting =============

type SortKey = "pid" | "name" | "cpu" | "memory_kb" | "gpu";
const sortKey = ref<SortKey>("cpu");
const sortAsc = ref(false);

function toggleSort(key: SortKey) {
  if (sortKey.value === key) {
    sortAsc.value = !sortAsc.value;
  } else {
    sortKey.value = key;
    sortAsc.value = false;
  }
}

const sortedRows = computed(() => {
  const sorted = [...rows.value].sort((a, b) => {
    const aVal = a[sortKey.value];
    const bVal = b[sortKey.value];
    if (typeof aVal === "string" && typeof bVal === "string") {
      return aVal.localeCompare(bVal);
    }
    return (aVal as number) - (bVal as number);
  });
  return sortAsc.value ? sorted : sorted.reverse();
});

function getSortIcon(key: SortKey): string {
  if (sortKey.value !== key) return "⇅";
  return sortAsc.value ? "↑" : "↓";
}

// ============= Monitor Functions =============

function updateWatchlist() {
  watchlist.value = watchlistInput.value
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  refreshProcesses();
}

async function refreshProcesses() {
  if (watchlist.value.length === 0) {
    rows.value = [];
    groupedRows.value = [];
    return;
  }
  try {
    if (viewMode.value === "grouped") {
      groupedRows.value = await invoke<ProcessGroup[]>("grouped_processes", {
        names: watchlist.value,
      });
    } else {
      rows.value = await invoke<ProcRow[]>("watched_processes", {
        names: watchlist.value,
      });
    }
  } catch (e) {
    console.error("Error fetching processes:", e);
  }
}

async function refreshSystemStats() {
  try {
    systemStats.value = await invoke<SystemStats>("get_system_stats");
  } catch (e) {
    console.error("Error fetching system stats:", e);
  }
}

async function kill(pid: number, name: string) {
  if (!confirm(`End task: ${name} (PID ${pid})?`)) return;

  isLoading.value = true;
  statusMessage.value = "";
  try {
    const result = await invoke<string>("kill_pid", { pid });
    statusMessage.value = result;
    await refreshProcesses();
  } catch (e: unknown) {
    statusMessage.value = `Error: ${e}`;
  } finally {
    isLoading.value = false;
  }
}

async function killGroup(name: string, count: number) {
  if (!confirm(`End all ${count} processes of "${name}"?`)) return;

  isLoading.value = true;
  statusMessage.value = "";
  try {
    const result = await invoke<string>("kill_process_group", { name });
    statusMessage.value = result;
    await refreshProcesses();
  } catch (e: unknown) {
    statusMessage.value = `Error: ${e}`;
  } finally {
    isLoading.value = false;
  }
}

// Process Modal
const showProcessModal = ref(false);
const allProcesses = ref<ProcessGroup[]>([]);
const processSearch = ref("");

const filteredProcesses = computed(() => {
  if (!processSearch.value) return allProcesses.value;
  const lower = processSearch.value.toLowerCase();
  return allProcesses.value.filter(p => p.name.toLowerCase().includes(lower));
});

async function openProcessModal() {
  try {
    isLoading.value = true;
    allProcesses.value = await invoke<ProcessGroup[]>("get_all_process_list");
    showProcessModal.value = true;
    processSearch.value = "";
  } catch (e) {
    console.error("Error fetching process list:", e);
  } finally {
    isLoading.value = false;
  }
}

function selectProcess(name: string) {
  newBlacklistName.value = name;
  showProcessModal.value = false;
  // Focus on add button or just ready to add
}

function toggleViewMode() {
  viewMode.value = viewMode.value === "grouped" ? "detailed" : "grouped";
  refreshProcesses();
}

// ============= Blacklist Functions =============

async function refreshBlacklist() {
  try {
    blacklist.value = await invoke<BlacklistEntry[]>("get_blacklist");
  } catch (e) {
    console.error("Error fetching blacklist:", e);
  }
}

async function refreshActivityLogs() {
  try {
    activityLogs.value = await invoke<ActivityLog[]>("get_activity_logs");
  } catch (e) {
    console.error("Error fetching activity logs:", e);
  }
}

async function addToBlacklist() {
  if (!newBlacklistName.value.trim()) return;

  blacklistStatus.value = "";
  try {
    await invoke<string>("add_to_blacklist", {
      name: newBlacklistName.value.trim(),
      autoKill: newAutoKill.value,
      cpuThreshold: newCpuEnabled.value ? newCpuThreshold.value : 101,
    });
    
    // Set GPU threshold
    const gpuThresh = newGpuEnabled.value ? newGpuThreshold.value : 101;
    await invoke("set_gpu_threshold", {
      name: newBlacklistName.value.trim(),
      threshold: gpuThresh
    });

    blacklistStatus.value = `Success: Added ${newBlacklistName.value.trim()}`;
    newBlacklistName.value = "";
    // Reset defaults
    newCpuEnabled.value = true;
    newCpuThreshold.value = 30;
    newGpuEnabled.value = false;
    newGpuThreshold.value = 50;
    await refreshBlacklist();
  } catch (e: unknown) {
    blacklistStatus.value = `Error: ${e}`;
  }
}

async function removeFromBlacklist(name: string) {
  if (!confirm(`Remove "${name}" from blacklist?`)) return;

  try {
    await invoke<string>("remove_from_blacklist", { name });
    await refreshBlacklist();
  } catch (e) {
    console.error("Error removing from blacklist:", e);
  }
}

async function toggleAutoKill(name: string) {
  try {
    await invoke<boolean>("toggle_auto_kill", { name });
    await refreshBlacklist();
  } catch (e) {
    console.error("Error toggling auto-kill:", e);
  }
}

async function toggleLog(name: string) {
  try {
    await invoke<boolean>("toggle_blacklist_log", { name });
    await refreshBlacklist();
  } catch (e) {
    console.error("Error toggling log:", e);
  }
}

async function toggleLogKillsOnly(name: string) {
  try {
    await invoke<boolean>("toggle_log_kills_only", { name });
    await refreshBlacklist();
  } catch (e) {
    console.error("Error toggling kills-only log:", e);
  }
}

async function setCpuThreshold(name: string, threshold: number) {
  try {
    await invoke<number>("set_cpu_threshold", { name, threshold });
    await refreshBlacklist();
  } catch (e) {
    console.error("Error setting CPU threshold:", e);
  }
}

async function setGpuThreshold(name: string, threshold: number) {
  try {
    await invoke<number>("set_gpu_threshold", { name, threshold });
    await refreshBlacklist();
  } catch (e) {
    console.error("Error setting GPU threshold:", e);
  }
}

// Helpers for Blacklist Cards
async function toggleEntryCpu(entry: BlacklistEntry) {
  const isEnabled = entry.cpu_threshold <= 100;
  // If currently Enabled (<=100), Toggle OFF -> Set to 101
  // If currently Disabled (101), Toggle ON  -> Set to 30 (default)
  const newVal = isEnabled ? 101 : 30;
  await setCpuThreshold(entry.name, newVal);
}

async function toggleEntryGpu(entry: BlacklistEntry) {
  const isEnabled = entry.gpu_threshold <= 100;
  // If currently Enabled, Toggle OFF -> 101
  // If currently Disabled, Toggle ON -> 50
  const newVal = isEnabled ? 101 : 50;
  await setGpuThreshold(entry.name, newVal);
}

async function clearLogs() {
  if (!confirm("Clear all activity logs?")) return;
  try {
    await invoke<string>("clear_activity_logs");
    await refreshActivityLogs();
  } catch (e) {
    console.error("Error clearing logs:", e);
  }
}

async function checkBlacklist() {
  try {
    await invoke<ActivityLog[]>("check_and_kill_blacklist");
    await refreshActivityLogs();
    await refreshBlacklist(); // Refresh to get updated kill counts
  } catch (e) {
    console.error("Error checking blacklist:", e);
  }
}

// ============= Lifecycle =============

async function refreshAll() {
  await Promise.all([
    refreshProcesses(),
    refreshSystemStats(),
    checkBlacklist(),
  ]);
}

onMounted(async () => {
  // Check admin status first
  try {
    isAdmin.value = await invoke<boolean>("is_running_as_admin");
  } catch (e) {
    console.error("Error checking admin status:", e);
  }

  await refreshBlacklist();
  await refreshActivityLogs();
  await refreshAll();
  timer = setInterval(refreshAll, 1000);
});

onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});

function getUsageColor(percent: number): string {
  if (percent >= 90) return "var(--danger)";
  if (percent >= 70) return "var(--warning)";
  return "var(--success)";
}
</script>

<template>
  <main class="container">
    <!-- Tab Navigation -->
    <nav class="tabs">
      <button :class="['tab', { active: activeTab === 'monitor' }]" @click="activeTab = 'monitor'">
        🖥️ Monitor
      </button>
      <button :class="['tab', { active: activeTab === 'blacklist' }]" @click="activeTab = 'blacklist'">
        🚫 Blacklist
        <span v-if="blacklist.length > 0" class="badge">{{ blacklist.length }}</span>
      </button>
    </nav>

    <!-- Monitor Tab -->
    <div v-show="activeTab === 'monitor'">
      <header class="header">
        <h1>🖥️ System Monitor</h1>
        <p class="subtitle">Monitor system resources and manage processes</p>
      </header>

      <!-- System Stats Dashboard -->
      <section class="stats-grid" v-if="systemStats">
        <div class="stat-card">
          <div class="stat-icon">⚡</div>
          <div class="stat-content">
            <div class="stat-label">CPU Usage</div>
            <div class="stat-value">{{ systemStats.cpu_usage.toFixed(1) }}%</div>
            <div class="stat-bar">
              <div class="stat-bar-fill" :style="{
                width: `${Math.min(systemStats.cpu_usage, 100)}%`,
                backgroundColor: getUsageColor(systemStats.cpu_usage),
              }"></div>
            </div>
          </div>
        </div>

        <div class="stat-card">
          <div class="stat-icon">🧠</div>
          <div class="stat-content">
            <div class="stat-label">Memory</div>
            <div class="stat-value">
              {{ systemStats.memory_used_gb.toFixed(1) }} /
              {{ systemStats.memory_total_gb.toFixed(1) }} GB
            </div>
            <div class="stat-bar">
              <div class="stat-bar-fill" :style="{
                width: `${Math.min(systemStats.memory_percent, 100)}%`,
                backgroundColor: getUsageColor(systemStats.memory_percent),
              }"></div>
            </div>
          </div>
        </div>
      </section>

      <!-- Disk Info -->
      <section class="disks-section" v-if="systemStats && systemStats.disks.length > 0">
        <h2 class="section-title">💾 Disks</h2>
        <div class="disks-grid">
          <div class="disk-card" v-for="disk in systemStats.disks" :key="disk.mount_point">
            <div class="disk-header">
              <span class="disk-name">{{ disk.mount_point }}</span>
              <span class="disk-label">{{ disk.name || "Local Disk" }}</span>
            </div>
            <div class="disk-usage">
              {{ disk.used_gb.toFixed(1) }} / {{ disk.total_gb.toFixed(1) }} GB
            </div>
            <div class="stat-bar">
              <div class="stat-bar-fill" :style="{
                width: `${Math.min(disk.usage_percent, 100)}%`,
                backgroundColor: getUsageColor(disk.usage_percent),
              }"></div>
            </div>
            <div class="disk-free">{{ disk.free_gb.toFixed(1) }} GB free</div>
          </div>
        </div>
      </section>

      <!-- Watchlist Section -->
      <section class="watchlist-section">
        <h2 class="section-title">🔍 Process Watchlist</h2>
        <div class="input-row">
          <input id="watchlist-input" v-model="watchlistInput"
            placeholder="Enter process names (comma-separated), e.g. chrome, notepad, code"
            @keyup.enter="updateWatchlist" />
          <button class="btn-primary" @click="updateWatchlist">Watch</button>
        </div>
        <div class="tags" v-if="watchlist.length > 0">
          <span v-for="name in watchlist" :key="name" class="tag">{{ name }}</span>
        </div>
      </section>

      <section class="status-bar" v-if="statusMessage">
        <span :class="statusMessage.startsWith('Error') ? 'error' : 'success'">
          {{ statusMessage }}
        </span>
      </section>

      <!-- Process Table -->
      <section class="table-section" v-if="watchlist.length > 0">
        <div class="table-header">
          <h2>Running Processes</h2>
          <div class="table-controls">
            <button class="btn-toggle-view" @click="toggleViewMode">
              {{ viewMode === 'grouped' ? '📊 Grouped' : '📋 Detailed' }}
            </button>
            <span class="count">
              {{ viewMode === 'grouped' ? groupedRows.length + ' groups' : rows.length + ' processes' }}
            </span>
          </div>
        </div>

        <!-- Grouped View (like Task Manager) -->
        <div class="table-wrapper" v-if="viewMode === 'grouped' && groupedRows.length > 0">
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Processes</th>
                <th>CPU %</th>
                <th>GPU %</th>
                <th>Memory</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="group in groupedRows" :key="group.name">
                <td class="name">
                  {{ group.name }}
                </td>
                <td class="process-count">
                  <span class="count-badge">{{ group.process_count }}</span>
                </td>
                <td class="cpu">{{ group.total_cpu.toFixed(1) }}%</td>
                <td class="gpu">{{ group.total_gpu?.toFixed(1) || '0.0' }}%</td>
                <td class="memory">{{ (group.total_memory_kb / 1024).toFixed(1) }} MB</td>
                <td>
                  <button class="btn-danger" @click="killGroup(group.name, group.process_count)" :disabled="isLoading">
                    End All
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Detailed View (individual processes) -->
        <div class="table-wrapper" v-else-if="viewMode === 'detailed' && rows.length > 0">
          <table>
            <thead>
              <tr>
                <th class="sortable" @click="toggleSort('pid')">
                  PID <span class="sort-icon">{{ getSortIcon('pid') }}</span>
                </th>
                <th class="sortable" @click="toggleSort('name')">
                  Name <span class="sort-icon">{{ getSortIcon('name') }}</span>
                </th>
                <th class="sortable" @click="toggleSort('cpu')">
                  CPU % <span class="sort-icon">{{ getSortIcon('cpu') }}</span>
                </th>
                <th class="sortable" @click="toggleSort('gpu')">
                  GPU % <span class="sort-icon">{{ getSortIcon('gpu') }}</span>
                </th>
                <th class="sortable" @click="toggleSort('memory_kb')">
                  Memory (KB) <span class="sort-icon">{{ getSortIcon('memory_kb') }}</span>
                </th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="proc in sortedRows" :key="proc.pid">
                <td class="pid">{{ proc.pid }}</td>
                <td class="name">{{ proc.name }}</td>
                <td class="cpu">{{ proc.cpu.toFixed(1) }}%</td>
                <td class="gpu">{{ proc.gpu?.toFixed(1) || '0.0' }}%</td>
                <td class="memory">{{ proc.memory_kb.toLocaleString() }}</td>
                <td>
                  <button class="btn-danger" @click="kill(proc.pid, proc.name)" :disabled="isLoading">
                    End Task
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="empty-state" v-else>
          <p>No matching processes found.</p>
          <p class="hint">Make sure the process names in your watchlist are running.</p>
        </div>
      </section>

      <section class="empty-state" v-else-if="watchlist.length === 0">
        <p>Enter process names above to start monitoring.</p>
      </section>
    </div>

    <!-- Blacklist Tab -->
    <div v-show="activeTab === 'blacklist'">
      <header class="header">
        <h1>🚫 Blacklist Manager</h1>
        <p class="subtitle">Block processes and auto-kill when detected</p>
      </header>

      <!-- Admin Warning -->
      <div class="admin-warning" v-if="!isAdmin">
        <span class="admin-warning-icon">⚠️</span>
        <span class="admin-warning-text">กำลังรันโดยไม่มีสิทธิ์ Administrator - บาง Process อาจ kill ไม่ได้</span>
      </div>

      <div class="admin-status" v-else>
        <span class="admin-ok-icon">🛡️</span>
        <span class="admin-ok-text">รันด้วยสิทธิ์ Administrator - สามารถ kill ทุก Process ได้</span>
      </div>

      <!-- Add to Blacklist -->
      <section class="watchlist-section">
        <h2 class="section-title">➕ Add to Blacklist</h2>
        <div class="input-row">
          <input v-model="newBlacklistName" placeholder="Enter process name, e.g. searchindexer"
            @keyup.enter="addToBlacklist" />
          <label class="checkbox-label">
            <input type="checkbox" v-model="newAutoKill" />
            Auto-Kill
          </label>
          <button class="btn-primary" @click="addToBlacklist">Add</button>
          <button class="btn-secondary" @click="openProcessModal" title="Browse Running Processes">
            📂 Browse
          </button>
        </div>
        <!-- Triggers -->
        <div class="triggers-container" v-if="newAutoKill">
          <div class="trigger-box">
            <div class="trigger-header">
              <label class="checkbox-label">
                <input type="checkbox" v-model="newCpuEnabled" />
                Monitor CPU
              </label>
              <span class="status-text">{{ newCpuEnabled ? 'ON' : 'OFF' }}</span>
            </div>
            <div class="trigger-body" v-if="newCpuEnabled">
              <input type="range" v-model.number="newCpuThreshold" min="0" max="100" step="5" class="slider" />
              <span class="threshold-value">{{ newCpuThreshold }}%</span>
            </div>
            <div class="trigger-hint" v-if="newCpuEnabled">(0 = Always Kill)</div>
          </div>

          <div class="trigger-box">
            <div class="trigger-header">
              <label class="checkbox-label">
                <input type="checkbox" v-model="newGpuEnabled" />
                Monitor GPU
              </label>
              <span class="status-text">{{ newGpuEnabled ? 'ON' : 'OFF' }}</span>
            </div>
            <div class="trigger-body" v-if="newGpuEnabled">
              <input type="range" v-model.number="newGpuThreshold" min="0" max="100" step="5" class="slider" />
              <span class="threshold-value">{{ newGpuThreshold }}%</span>
            </div>
          </div>
        </div>
      </section>

      <section class="status-bar" v-if="blacklistStatus">
        <span :class="blacklistStatus.startsWith('Error') ? 'error' : 'success'">
          {{ blacklistStatus }}
        </span>
      </section>

      <!-- Blacklist Cards -->
      <section class="blacklist-cards" v-if="blacklist.length > 0">
        <div class="cards-header">
          <h2>🔒 Blocked Processes</h2>
          <span class="count">{{ blacklist.length }} entries</span>
        </div>

        <div class="cards-grid">
          <div v-for="entry in blacklist" :key="entry.name" class="blacklist-card" :class="{ active: entry.auto_kill }">
            <div class="card-header">
              <span class="process-name">{{ entry.name }}</span>
              <button class="btn-remove" @click="removeFromBlacklist(entry.name)" title="Remove">
                ✕
              </button>
            </div>

            <div class="card-body">
              <!-- Auto-Kill Toggle -->
              <div class="control-row">
                <span class="control-label">Auto-Kill</span>
                <button :class="['toggle-switch', { on: entry.auto_kill }]" @click="toggleAutoKill(entry.name)">
                  <span class="toggle-slider"></span>
                  <span class="toggle-text">{{ entry.auto_kill ? 'ON' : 'OFF' }}</span>
                </button>
              </div>

              <!-- Triggers Area -->
              <div class="triggers-area" v-if="entry.auto_kill">
                <!-- CPU Trigger -->
                <div class="trigger-item">
                  <div class="trigger-top">
                    <span class="trigger-label">CPU Monitor</span>
                    <button :class="['toggle-switch small', { on: entry.cpu_threshold <= 100 }]"
                      @click="toggleEntryCpu(entry)">
                      <span class="toggle-slider"></span>
                    </button>
                  </div>
                  <div class="trigger-control" v-if="entry.cpu_threshold <= 100">
                    <input type="range" :value="entry.cpu_threshold"
                      @input="setCpuThreshold(entry.name, Number(($event.target as HTMLInputElement).value))" min="0"
                      max="100" step="5" class="slider-compact" />
                    <span class="val">{{ entry.cpu_threshold === 0 ? 'Always' : entry.cpu_threshold + '%' }}</span>
                  </div>
                </div>

                <!-- GPU Trigger -->
                <div class="trigger-item">
                  <div class="trigger-top">
                    <span class="trigger-label">GPU Monitor</span>
                    <button :class="['toggle-switch small', { on: entry.gpu_threshold <= 100 }]"
                      @click="toggleEntryGpu(entry)">
                      <span class="toggle-slider"></span>
                    </button>
                  </div>
                  <div class="trigger-control" v-if="entry.gpu_threshold <= 100">
                    <input type="range" :value="entry.gpu_threshold"
                      @input="setGpuThreshold(entry.name, Number(($event.target as HTMLInputElement).value))" min="0"
                      max="100" step="5" class="slider-compact" />
                    <span class="val">{{ entry.gpu_threshold }}%</span>
                  </div>
                </div>
              </div>

              <!-- Log Toggle -->
              <div class="control-row">
                <div style="display: flex; flex-direction: column;">
                  <span class="control-label">Record Log</span>
                  <span v-if="entry.log_enabled && entry.log_kills_only" class="hint"
                    style="font-size: 9px; color: var(--danger);">Kills only</span>
                </div>
                <div style="display: flex; gap: 8px; align-items: center;">
                  <button v-if="entry.log_enabled"
                    :class="['toggle-switch small secondary', { on: entry.log_kills_only }]"
                    @click="toggleLogKillsOnly(entry.name)" title="Log kills only">
                    <span class="toggle-slider"></span>
                    <span class="toggle-text" style="font-size: 8px;">KILLS</span>
                  </button>
                  <button :class="['toggle-switch small', { on: entry.log_enabled }]" @click="toggleLog(entry.name)">
                    <span class="toggle-slider"></span>
                  </button>
                </div>
              </div>
            </div>

            <div class="card-footer">
              <div class="kill-stats">
                <span class="kill-icon">💀</span>
                <span class="kill-number">{{ entry.kill_count }}</span>
                <span class="kill-label">kills</span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section class="empty-state" v-else>
        <p>No processes in blacklist.</p>
        <p class="hint">Add process names above to start blocking.</p>
      </section>

      <!-- Activity Logs -->
      <section class="table-section" style="margin-top: 20px;">
        <div class="table-header">
          <h2>📝 Activity Log</h2>
          <div style="display: flex; gap: 10px; align-items: center;">
            <span class="count">{{ activityLogs.length }} events</span>
            <button class="btn-small" @click="clearLogs" v-if="activityLogs.length > 0">
              Clear
            </button>
          </div>
        </div>

        <div class="table-wrapper" v-if="activityLogs.length > 0">
          <table>
            <thead>
              <tr>
                <th>Time</th>
                <th>Process</th>
                <th>PID</th>
                <th>CPU</th>
                <th>GPU</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(log, idx) in activityLogs" :key="idx" :class="{ 'row-killed': log.was_killed }">
                <td class="date">{{ log.detected_at }}</td>
                <td class="name">{{ log.name }}</td>
                <td class="pid">{{ log.pid }}</td>
                <td class="cpu">{{ log.cpu_usage.toFixed(1) }}%</td>
                <td class="gpu">{{ log.gpu_usage?.toFixed(1) || '0.0' }}%</td>
                <td>
                  <span :class="['status-badge', log.was_killed ? 'killed' : 'detected']" :title="log.reason">
                    {{ log.was_killed ? '🔴 Killed' : '👁️ ' + log.reason }}
                  </span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="empty-state" v-else>
          <p>No activity recorded yet.</p>
        </div>
      </section>
    </div>

    <!-- Process Selection Modal -->
    <div class="modal-overlay" v-if="showProcessModal" @click.self="showProcessModal = false">
      <div class="modal-window">
        <div class="modal-header">
          <h3>Select Process to Block</h3>
          <button class="btn-close" @click="showProcessModal = false">✕</button>
        </div>

        <div class="modal-search">
          <input v-model="processSearch" placeholder="Search process name..." autofocus />
        </div>

        <div class="modal-list-selector">
          <table class="modal-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Count</th>
                <th>Total CPU</th>
                <th>Total Mem</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="proc in filteredProcesses" :key="proc.name" @click="selectProcess(proc.name)"
                class="modal-row">
                <td class="name-col">{{ proc.name }}</td>
                <td class="count-col"><span class="badge">{{ proc.process_count }}</span></td>
                <td>{{ proc.total_cpu.toFixed(1) }}%</td>
                <td>{{ (proc.total_memory_kb / 1024).toFixed(0) }} MB</td>
              </tr>
            </tbody>
          </table>
          <div v-if="filteredProcesses.length === 0" class="empty-state small">
            No processes found
          </div>
        </div>
      </div>
    </div>
  </main>
</template>

<style>
:root {
  --bg-primary: #0d1117;
  --bg-secondary: #161b22;
  --bg-tertiary: #21262d;
  --text-primary: #f0f6fc;
  --text-secondary: #8b949e;
  --accent: #58a6ff;
  --accent-hover: #79b8ff;
  --danger: #f85149;
  --danger-hover: #ff7b72;
  --success: #3fb950;
  --warning: #d29922;
  --border: #30363d;

  font-family: "Inter", "Segoe UI", system-ui, -apple-system, sans-serif;
  font-size: 14px;
  line-height: 1.5;
  color: var(--text-primary);
  background: var(--bg-primary);
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

.container {
  max-width: 1000px;
  margin: 0 auto;
  padding: 20px;
  min-height: 100vh;
}

/* Tabs */
.tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 20px;
  background: var(--bg-secondary);
  padding: 4px;
  border-radius: 10px;
  border: 1px solid var(--border);
}

.tab {
  flex: 1;
  padding: 12px 20px;
  background: transparent;
  border: none;
  border-radius: 8px;
  color: var(--text-secondary);
  font-weight: 600;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.tab:hover {
  color: var(--text-primary);
  background: var(--bg-tertiary);
}

.tab.active {
  background: var(--accent);
  color: #fff;
}

.badge {
  background: rgba(255, 255, 255, 0.2);
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 11px;
}

/* Admin Status Banners */
.admin-warning {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  background: rgba(210, 153, 34, 0.15);
  border: 1px solid var(--warning);
  border-radius: 8px;
  margin-bottom: 16px;
  animation: pulse-warning 2s infinite;
}

@keyframes pulse-warning {

  0%,
  100% {
    opacity: 1;
  }

  50% {
    opacity: 0.7;
  }
}

.admin-warning-icon {
  font-size: 16px;
}

.admin-warning-text {
  font-size: 13px;
  color: var(--warning);
  font-weight: 500;
}

.admin-status {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  background: rgba(63, 185, 80, 0.15);
  border: 1px solid var(--success);
  border-radius: 8px;
  margin-bottom: 16px;
}

.admin-ok-icon {
  font-size: 16px;
}

.admin-ok-text {
  font-size: 13px;
  color: var(--success);
  font-weight: 500;
}

.header {
  text-align: center;
  margin-bottom: 24px;
}

.header h1 {
  font-size: 24px;
  font-weight: 700;
  background: linear-gradient(135deg, var(--accent), #a371f7);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.subtitle {
  color: var(--text-secondary);
  margin-top: 4px;
  font-size: 13px;
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 12px;
  color: var(--text-primary);
}

/* Stats Grid */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 14px;
  margin-bottom: 18px;
}

.stat-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 14px;
  display: flex;
  gap: 12px;
  align-items: flex-start;
}

.stat-icon {
  font-size: 24px;
}

.stat-content {
  flex: 1;
}

.stat-label {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.stat-value {
  font-size: 18px;
  font-weight: 700;
  margin: 2px 0 6px;
}

.stat-bar {
  height: 5px;
  background: var(--bg-tertiary);
  border-radius: 3px;
  overflow: hidden;
}

.stat-bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.3s ease;
}

/* Disks Section */
.disks-section {
  margin-bottom: 18px;
}

.disks-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 10px;
}

.disk-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 12px;
}

.disk-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 6px;
}

.disk-name {
  font-weight: 600;
  font-size: 14px;
}

.disk-label {
  font-size: 10px;
  color: var(--text-secondary);
}

.disk-usage {
  font-size: 12px;
  color: var(--text-secondary);
  margin-bottom: 4px;
}

.disk-free {
  font-size: 10px;
  color: var(--text-secondary);
  margin-top: 4px;
}

/* Watchlist Section */
.watchlist-section {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 16px;
  margin-bottom: 16px;
}

.input-row {
  display: flex;
  gap: 10px;
  align-items: center;
}

.input-row input[type="text"],
.input-row input:not([type]) {
  flex: 1;
  padding: 10px 14px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-primary);
  font-size: 13px;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.input-row input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(88, 166, 255, 0.15);
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
}

.checkbox-label input[type="checkbox"] {
  width: 16px;
  height: 16px;
  accent-color: var(--accent);
}

.btn-primary {
  padding: 10px 18px;
  background: var(--accent);
  color: #fff;
  border: none;
  border-radius: 6px;
  font-weight: 600;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.2s, transform 0.1s;
  white-space: nowrap;
}

.btn-primary:hover {
  background: var(--accent-hover);
}

.btn-primary:active {
  transform: scale(0.98);
}

.btn-small {
  padding: 6px 12px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-small:hover {
  background: var(--danger);
  color: #fff;
  border-color: var(--danger);
}

.tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 10px;
}

.tag {
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  padding: 3px 10px;
  border-radius: 14px;
  font-size: 11px;
  color: var(--accent);
}

.status-bar {
  padding: 10px 14px;
  border-radius: 6px;
  margin-bottom: 16px;
  background: var(--bg-secondary);
  border: 1px solid var(--border);
}

.status-bar .success {
  color: var(--success);
}

.status-bar .error {
  color: var(--danger);
}

/* Table Section */
.table-section {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: hidden;
}

.table-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.table-header h2 {
  font-size: 13px;
  font-weight: 600;
}

.count {
  background: var(--bg-tertiary);
  padding: 3px 10px;
  border-radius: 14px;
  font-size: 11px;
  color: var(--text-secondary);
}

.table-wrapper {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
}

thead {
  background: var(--bg-tertiary);
}

th {
  text-align: left;
  padding: 10px 14px;
  font-weight: 600;
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
}

th.sortable {
  cursor: pointer;
  user-select: none;
  transition: color 0.2s, background 0.2s;
}

th.sortable:hover {
  color: var(--accent);
  background: rgba(88, 166, 255, 0.08);
}

.sort-icon {
  margin-left: 4px;
  font-size: 10px;
  opacity: 0.7;
}

td {
  padding: 10px 14px;
  border-top: 1px solid var(--border);
  font-size: 13px;
}

tr:hover {
  background: rgba(88, 166, 255, 0.04);
}

.pid {
  font-family: "Fira Code", "Cascadia Code", monospace;
  color: var(--accent);
  font-size: 12px;
}

.name {
  font-weight: 500;
}

.cpu,
.gpu,
.memory {
  font-family: "Fira Code", "Cascadia Code", monospace;
  color: var(--text-secondary);
  font-size: 12px;
}

.date {
  font-size: 11px;
  color: var(--text-secondary);
}

.kill-count {
  font-family: "Fira Code", "Cascadia Code", monospace;
  color: var(--danger);
  font-weight: 600;
}

.btn-danger {
  padding: 6px 12px;
  background: transparent;
  color: var(--danger);
  border: 1px solid var(--danger);
  border-radius: 4px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-danger:hover:not(:disabled) {
  background: var(--danger);
  color: #fff;
}

.btn-danger:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-toggle {
  padding: 6px 12px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-toggle.active {
  background: rgba(248, 81, 73, 0.15);
  color: var(--danger);
  border-color: var(--danger);
}

.status-badge {
  padding: 4px 10px;
  border-radius: 12px;
  font-size: 11px;
  font-weight: 600;
}

.status-badge.killed {
  background: rgba(248, 81, 73, 0.15);
  color: var(--danger);
}

.status-badge.detected {
  background: rgba(88, 166, 255, 0.15);
  color: var(--accent);
}

.empty-state {
  padding: 36px 20px;
  text-align: center;
}

.empty-state p {
  color: var(--text-secondary);
}

.hint {
  font-size: 11px;
  color: var(--text-secondary);
}

/* Threshold Controls */
.threshold-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 12px;
  padding: 10px 14px;
  background: var(--bg-tertiary);
  border-radius: 6px;
}

.threshold-row label {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.slider {
  flex: 1;
  max-width: 200px;
  height: 6px;
  appearance: none;
  background: var(--border);
  border-radius: 3px;
  cursor: pointer;
}

.slider::-webkit-slider-thumb {
  appearance: none;
  width: 16px;
  height: 16px;
  background: var(--accent);
  border-radius: 50%;
  cursor: pointer;
}

.threshold-value {
  font-weight: 700;
  color: var(--accent);
  min-width: 40px;
}

.threshold-hint {
  font-size: 10px;
  color: var(--text-secondary);
}

.threshold-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.slider-small {
  width: 80px;
  height: 4px;
  appearance: none;
  background: var(--border);
  border-radius: 2px;
  cursor: pointer;
}

.slider-small::-webkit-slider-thumb {
  appearance: none;
  width: 12px;
  height: 12px;
  background: var(--accent);
  border-radius: 50%;
  cursor: pointer;
}

.text-muted {
  color: var(--text-secondary);
}

.row-killed {
  background: rgba(248, 81, 73, 0.06);
}

/* Grouped View Controls */
.table-controls {
  display: flex;
  align-items: center;
  gap: 12px;
}

.btn-toggle-view {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border: 1px solid var(--border);
  border-radius: 6px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-toggle-view:hover {
  background: var(--bg-secondary);
  border-color: var(--accent);
  color: var(--accent);
}

.count-badge {
  background: rgba(88, 166, 255, 0.15);
  color: var(--accent);
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 11px;
  font-weight: 700;
  font-family: "Fira Code", monospace;
}

.process-count {
  text-align: center;
}

/* ============= Blacklist Cards ============= */
.blacklist-cards {
  margin-top: 20px;
}

.cards-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.cards-header h2 {
  margin: 0;
  font-size: 16px;
  color: var(--text-primary);
}

.cards-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
}

.blacklist-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
  transition: all 0.2s;
}

.blacklist-card:hover {
  border-color: var(--text-secondary);
  transform: translateY(-2px);
}

.blacklist-card.active {
  border-color: var(--danger);
  box-shadow: 0 0 20px rgba(248, 81, 73, 0.15);
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: var(--bg-tertiary);
  border-bottom: 1px solid var(--border);
}

.process-name {
  font-weight: 700;
  font-size: 14px;
  color: var(--text-primary);
  font-family: "Fira Code", monospace;
}

.btn-remove {
  width: 24px;
  height: 24px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 4px;
  font-size: 12px;
  transition: all 0.2s;
}

.btn-remove:hover {
  background: rgba(248, 81, 73, 0.15);
  color: var(--danger);
}

.card-body {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.control-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.control-label {
  font-size: 12px;
  color: var(--text-secondary);
}

/* Toggle Switch */
.toggle-switch {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px 4px 4px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  border-radius: 20px;
  cursor: pointer;
  transition: all 0.2s;
}

.toggle-switch .toggle-slider {
  width: 32px;
  height: 18px;
  background: var(--border);
  border-radius: 9px;
  position: relative;
  transition: all 0.2s;
}

.toggle-switch .toggle-slider::after {
  content: '';
  position: absolute;
  width: 14px;
  height: 14px;
  background: var(--text-secondary);
  border-radius: 50%;
  top: 2px;
  left: 2px;
  transition: all 0.2s;
}

.toggle-switch.on .toggle-slider {
  background: var(--danger);
}

.toggle-switch.on .toggle-slider::after {
  left: 16px;
  background: white;
}

.toggle-switch .toggle-text {
  font-size: 10px;
  font-weight: 700;
  color: var(--text-secondary);
}

.toggle-switch.on .toggle-text {
  color: var(--danger);
}

.toggle-switch.small {
  padding: 2px;
}

.toggle-switch.small .toggle-slider {
  width: 28px;
  height: 16px;
}

.toggle-switch.small .toggle-slider::after {
  width: 12px;
  height: 12px;
}

.toggle-switch.small.on .toggle-slider::after {
  left: 14px;
}

.toggle-switch.secondary.on .toggle-slider {
  background: var(--warning);
}

.toggle-switch.secondary.on .toggle-text {
  color: var(--warning);
}

/* Threshold Control */
.threshold-control {
  display: flex;
  align-items: center;
  gap: 8px;
}

.slider-compact {
  width: 80px;
  height: 4px;
  appearance: none;
  background: var(--border);
  border-radius: 2px;
  cursor: pointer;
}

.slider-compact::-webkit-slider-thumb {
  appearance: none;
  width: 14px;
  height: 14px;
  background: var(--accent);
  border-radius: 50%;
  cursor: pointer;
}

.threshold-badge {
  background: rgba(248, 81, 73, 0.15);
  color: var(--danger);
  padding: 2px 8px;
  border-radius: 8px;
  font-size: 10px;
  font-weight: 700;
  min-width: 45px;
  text-align: center;
}

/* Card Footer */
.card-footer {
  padding: 10px 16px;
  background: var(--bg-tertiary);
  border-top: 1px solid var(--border);
}

.kill-stats {
  display: flex;
  align-items: center;
  gap: 6px;
}

.kill-icon {
  font-size: 14px;
}

.kill-number {
  font-size: 18px;
  font-weight: 700;
  color: var(--danger);
  font-family: "Fira Code", monospace;
}

.kill-label {
  font-size: 11px;
  color: var(--text-secondary);
}

/* ============= Modal ============= */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
}

.modal-window {
  background: var(--bg-primary);
  border: 1px solid var(--border);
  border-radius: 12px;
  width: 90%;
  max-width: 600px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  animation: slideUp 0.2s ease-out;
}

@keyframes slideUp {
  from {
    transform: translateY(20px);
    opacity: 0;
  }

  to {
    transform: translateY(0);
    opacity: 1;
  }
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px;
  border-bottom: 1px solid var(--border);
}

.modal-header h3 {
  margin: 0;
  font-size: 18px;
}

.btn-close {
  background: transparent;
  border: none;
  font-size: 20px;
  color: var(--text-secondary);
  cursor: pointer;
}

.btn-close:hover {
  color: var(--text-primary);
}

.modal-search {
  padding: 16px;
  border-bottom: 1px solid var(--border);
}

.modal-search input {
  width: 100%;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  padding: 10px;
  border-radius: 6px;
  color: var(--text-primary);
  font-size: 14px;
}

.modal-search input:focus {
  outline: none;
  border-color: var(--accent);
}

.modal-list-selector {
  flex: 1;
  overflow-y: auto;
  padding: 0;
}

.modal-table {
  width: 100%;
  border-collapse: collapse;
}

.modal-table th {
  text-align: left;
  padding: 12px 16px;
  color: var(--text-secondary);
  font-size: 12px;
  position: sticky;
  top: 0;
  background: var(--bg-primary);
  border-bottom: 1px solid var(--border);
}

.modal-row {
  cursor: pointer;
  transition: all 0.1s;
}

.modal-row:hover {
  background: var(--bg-secondary);
}

.modal-row td {
  padding: 10px 16px;
  border-bottom: 1px solid var(--border);
  color: var(--text-secondary);
}

.modal-row .name-col {
  color: var(--text-primary);
  font-weight: 500;
}

.empty-state.small {
  padding: 40px;
  text-align: center;
  color: var(--text-secondary);
}
</style>

<style>
/* Triggers Layout */
.triggers-container {
  display: flex;
  gap: 12px;
  margin-top: 12px;
}

.trigger-box {
  flex: 1;
  background: var(--bg-tertiary);
  border-radius: 8px;
  padding: 12px;
  border: 1px solid var(--border);
}

.trigger-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.status-text {
  font-size: 10px;
  font-weight: 700;
  color: var(--text-secondary);
}

.trigger-header input:checked ~ .status-text {
    color: var(--accent);
}

.trigger-body {
  display: flex;
  align-items: center;
  gap: 8px;
}

.trigger-hint {
    font-size: 10px;
    color: var(--text-secondary);
    margin-top: 4px;
    text-align: right;
}

/* Card Triggers */
.triggers-area {
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--bg-primary); /* Darker inner bg */
  padding: 10px;
  border-radius: 8px;
}

.trigger-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border);
}

.trigger-item:last-child {
    border-bottom: none;
    padding-bottom: 0;
}

.trigger-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.trigger-label {
    font-size: 11px;
    color: var(--text-secondary);
    font-weight: 600;
}

.trigger-control {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 4px;
}

.val {
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    min-width: 35px;
    text-align: right;
}
</style>
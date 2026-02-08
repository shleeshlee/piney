<script lang="ts">
    import { DatabaseBackup, Upload, Download, RotateCcw, AlertTriangle, FileUp, HardDriveDownload } from "lucide-svelte";
    import { Button } from "$lib/components/ui/button";
    import * as Card from "$lib/components/ui/card";
    import * as Tabs from "$lib/components/ui/tabs";
    import * as AlertDialog from "$lib/components/ui/alert-dialog";
    import { toast } from "svelte-sonner";
    import { cn } from "$lib/utils";
    import { API_BASE } from "$lib/api";
    import { onMount } from "svelte";
    import { breadcrumbs } from "$lib/stores/breadcrumb";

    let activeTab = "export";
    let isRestoreDialogOpen = false;
    let selectedFile: File | null = null;
    let fileInput: HTMLInputElement;

    import { downloadFile } from "$lib/utils/download";
    import RestartOverlay from "$lib/components/RestartOverlay.svelte";
    import { isTauri, invoke } from "@tauri-apps/api/core";
    import { backupExport } from "$lib/stores/backupExport";
    // import { relaunch } from "@tauri-apps/plugin-process"; // Soft restart now

    // --- 导出备份 ---
    let isRestarting = false;

    async function handleExport() {
        if ($backupExport.isExporting) return;
        
        const token = localStorage.getItem("auth_token");
        if (!token) {
            toast.error("未登录");
            return;
        }

        // Web 环境：使用原生浏览器下载（流式，不占内存）
        if (!isTauri()) {
            toast.info("正在准备下载...");
            // 通过 URL token 认证，浏览器原生下载
            window.location.href = `${API_BASE}/api/backup/export?token=${encodeURIComponent(token)}`;
            return;
        }

        // Tauri 环境：使用流式下载 + 进度显示
        backupExport.startExport();
        
        try {
            await downloadFile({
                filename: "piney_backup.piney",
                url: `${API_BASE}/api/backup/export`,
                type: "application/octet-stream",
                fetchOptions: {
                    headers: { Authorization: `Bearer ${token}` }
                },
                onProgress: (percent, downloaded, total) => {
                    backupExport.setProgress(downloaded, total, percent);
                }
            });
            
            backupExport.completeExport();
            
        } catch(e) {
            console.error(e);
            backupExport.failExport(String(e));
        }
    }

    // --- 重启与轮询逻辑 ---
    async function checkServerStatus() {
        let successCount = 0;
        const maxRetries = 60; // 60s timeout
        
        for (let i = 0; i < maxRetries; i++) {
            try {
                // 使用 fetch 而不是 axios/api 避免拦截器干扰
                const res = await fetch(`${API_BASE}/api/health`);
                if (res.ok) {
                    successCount++;
                    // 连续 2 次成功才视为服务就绪
                    if (successCount >= 2) return true;
                } else {
                    successCount = 0;
                }
            } catch (e) {
                successCount = 0;
            }
            await new Promise(r => setTimeout(r, 1000));
        }
        return false;
    }

    async function triggerRestart() {
        try {
            if (isTauri()) {
                await invoke('restart_server');
                // Soft restart: window stays open, polling handles the rest
            } else {
                // Web/Docker: 调用后端重启接口
                // 使用原生 fetch 避免全局错误处理
                const token = localStorage.getItem("auth_token");
                await fetch(`${API_BASE}/api/system/restart`, {
                    method: 'POST',
                    headers: token ? { Authorization: `Bearer ${token}` } : {}
                }).catch(() => {}); // 忽略请求中止错误（后端已重启）
            }
        } catch (e) {
            console.error("Restart trigger failed", e);
        }
    }

    function triggerFileInput() {
        fileInput.click();
    }

    function handleFileSelect(e: Event) {
        const target = e.target as HTMLInputElement;
        if (target.files && target.files.length > 0) {
            const file = target.files[0];
            if (!file.name.endsWith(".piney")) {
                toast.error("请选择 .piney 格式的备份文件");
                target.value = ""; // reset
                return;
            }
            selectedFile = file;
            isRestoreDialogOpen = true;
        }
    }

    let isRestoring = false;
    let showPostRestoreDialog = false;
    let restoredUsername = "";
    
    function handleLogoutAndRestart() {
        localStorage.removeItem("auth_token");
        window.location.href = "/login";
    }

    async function handleRestoreConfirm() {
        if (!selectedFile || isRestoring) return;
        
        isRestoreDialogOpen = false;
        isRestoring = true;
        localStorage.setItem("is_restarting", "true"); // 防止 401 导致跳转
        const loadingToast = toast.loading("正在上传并恢复数据...", { duration: Infinity });

        try {
            const token = localStorage.getItem("auth_token");
            const formData = new FormData();
            formData.append("backup", selectedFile);
            
            const res = await fetch(`${API_BASE}/api/backup/import`, {
                method: "POST",
                headers: token ? { Authorization: `Bearer ${token}` } : {},
                body: formData,
            });

            toast.dismiss(loadingToast);
            
            if (res.ok) {
                const data = await res.json();
                restoredUsername = data.username || "未知用户";
                
                if (data.token) {
                    localStorage.setItem("auth_token", data.token);
                }

                // === 开始平滑重启流程 ===
                isRestarting = true;
                
                // 1. 触发重启
                triggerRestart();

                // 2. 等待服务上线 (延迟 2秒开始检测，给后端一点关闭时间)
                await new Promise(r => setTimeout(r, 2000));
                const isBackOnline = await checkServerStatus();

                if (isBackOnline) {
                    toast.success(`恢复成功！欢迎回来，${restoredUsername}`);
                    window.location.href = "/";
                } else {
                    toast.error("服务重启超时，请手动刷新页面");
                    isRestarting = false;
                }
                return;
            } else {
                const text = await res.text();
                toast.error(`恢复失败: ${text}`);
            }
        } catch (e) {
            console.error(e);
            toast.dismiss(loadingToast);
            toast.error("恢复失败：网络错误");
        } finally {
            isRestoring = false;
            // Note: isRestarting 保持为 true 直到页面刷新，防止用户操作
            if (!isRestarting) { // 如果失败了（isRestarting被置为false），清除标志
                localStorage.removeItem("is_restarting");
            }
        }
    }

    function handleRestoreCancel() {
        isRestoreDialogOpen = false;
        selectedFile = null;
        if (fileInput) fileInput.value = "";
    }

    onMount(() => {
        breadcrumbs.set([
            { label: '数据备份' }
        ]);
    });
</script>

<div class="container py-8 space-y-8 max-w-4xl mx-auto">
    <div class="space-y-1">
        <h1 class="text-3xl font-bold tracking-tight">数据备份与恢复</h1>
        <p class="text-muted-foreground">
            管理小兄许数据，支持全量备份与一键恢复。
        </p>
    </div>

    <!-- Main Content Tabs -->
    <Tabs.Root bind:value={activeTab} class="w-full">
        <!-- 自定义 Tab 按钮，避免 bits-ui 在 Tauri 中的渲染问题 -->
        <div class="grid w-full grid-cols-2 mb-8 bg-muted p-1 rounded-lg">
            <button 
                type="button"
                onclick={() => activeTab = 'export'}
                class="text-base py-3 rounded-md flex items-center justify-center gap-2 transition-all {activeTab === 'export' ? 'bg-background text-foreground shadow-sm font-medium' : 'text-muted-foreground hover:text-foreground'}"
            >
                <DatabaseBackup class="h-4 w-4" />
                <span>导出数据</span>
            </button>
            <button 
                type="button"
                onclick={() => activeTab = 'import'}
                class="text-base py-3 rounded-md flex items-center justify-center gap-2 transition-all {activeTab === 'import' ? 'bg-background text-foreground shadow-sm font-medium' : 'text-muted-foreground hover:text-foreground'}"
            >
                <RotateCcw class="h-4 w-4" />
                <span>恢复数据</span>
            </button>
        </div>

        <!-- Tab 1: Export -->
        <Tabs.Content value="export" class="space-y-6 focus-visible:outline-none">
            <Card.Root class="border-primary/20 shadow-md">
                <Card.Header>
                    <Card.Title class="flex items-center gap-2 text-2xl">
                        <HardDriveDownload class="h-6 w-6 text-primary" />
                        系统全量备份
                    </Card.Title>
                    <Card.Description class="text-base mt-2">
                        将系统中的所有数据打包导出，生成 <code>.piney</code> 格式的备份文件。
                    </Card.Description>
                </Card.Header>
                <Card.Content class="space-y-6">
                    <div class="bg-muted/50 p-6 rounded-lg space-y-4 border border-border/50">
                        <h3 class="font-medium text-foreground">备份内容包含：</h3>
                        <ul class="grid grid-cols-2 gap-3 text-sm text-muted-foreground">
                            <li class="flex items-center gap-2">
                                <span class="h-1.5 w-1.5 rounded-full bg-primary/70"></span> 所有角色卡数据
                            </li>
                            <li class="flex items-center gap-2">
                                <span class="h-1.5 w-1.5 rounded-full bg-primary/70"></span> 所有上传的聊天记录
                            </li>
                            <li class="flex items-center gap-2">
                                <span class="h-1.5 w-1.5 rounded-full bg-primary/70"></span> 全局设置与偏好
                            </li>
                            <li class="flex items-center gap-2">
                                <span class="h-1.5 w-1.5 rounded-full bg-primary/70"></span> 世界书条目，等等...
                            </li>
                        </ul>
                    </div>

                    <div class="flex justify-end pt-4">
                        <Button 
                            size="lg" 
                            onclick={handleExport} 
                            disabled={$backupExport.isExporting}
                            class={cn(
                                "w-full sm:w-auto font-bold text-lg px-8 shadow-lg shadow-primary/20",
                                $backupExport.isExporting && "opacity-80"
                            )}
                        >
                            {#if $backupExport.isExporting}
                                <div class="mr-2 h-5 w-5 animate-spin rounded-full border-2 border-current border-t-transparent"></div>
                                {#if $backupExport.progress > 0}
                                    正在备份 {$backupExport.progress}%
                                {:else}
                                    正在备份...
                                {/if}
                            {:else}
                                <Download class="mr-2 h-5 w-5" />
                                立即备份数据
                            {/if}
                        </Button>
                    </div>
                </Card.Content>
            </Card.Root>
        </Tabs.Content>

        <!-- Tab 2: Import -->
        <Tabs.Content value="import" class="space-y-6 focus-visible:outline-none">
            <Card.Root class="border-destructive/20 shadow-md">
                <Card.Header>
                    <Card.Title class="flex items-center gap-2 text-2xl text-destructive/90">
                        <RotateCcw class="h-6 w-6" />
                        全量数据恢复
                    </Card.Title>
                    <Card.Description class="text-base mt-2">
                        导入 <code>.piney</code> 备份文件，覆盖当前系统状态。
                    </Card.Description>
                </Card.Header>
                <Card.Content class="space-y-6">
                    
                    <div class="rounded-lg border border-destructive/20 bg-destructive/5 p-4 text-destructive-foreground">
                        <div class="flex items-center gap-2 mb-2">
                            <AlertTriangle class="h-5 w-5 text-destructive" />
                            <h5 class="font-bold tracking-wide text-destructive">警告：高风险操作</h5>
                        </div>
                        <div class="ml-7 text-sm opacity-90 text-destructive/90 leading-relaxed">
                            恢复操作将<strong>完全清除</strong>当前的数据库、角色卡文件和所有配置，并用备份文件中的数据进行<strong>覆盖</strong>。<br/>
                            此操作一旦开始，<strong>无法撤销</strong>。请务必确认您已备份了当前的重要数据。
                        </div>
                    </div>

                    <input 
                        bind:this={fileInput}
                        type="file" 
                        accept=".piney" 
                        class="hidden" 
                        onchange={handleFileSelect}
                    />

                    <div class="flex flex-col items-center justify-center p-10 border-2 border-dashed border-border rounded-xl bg-card hover:bg-accent/30 transition-colors cursor-pointer group"
                         onclick={triggerFileInput} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && triggerFileInput()}
                    >
                        <div class="w-16 h-16 rounded-full bg-muted flex items-center justify-center mb-4 group-hover:scale-110 transition-transform duration-300">
                            <FileUp class="h-8 w-8 text-muted-foreground group-hover:text-primary transition-colors" />
                        </div>
                        <h3 class="text-lg font-semibold mb-1 group-hover:text-primary transition-colors">点击选择备份文件</h3>
                        <p class="text-sm text-muted-foreground">支持 .piney 格式</p>
                    </div>

                </Card.Content>
            </Card.Root>
        </Tabs.Content>
    </Tabs.Root>

    <!-- Restore Confirmation Dialog -->
    <RestartOverlay visible={isRestarting} />

    <AlertDialog.Root open={isRestoreDialogOpen} onOpenChange={handleRestoreCancel}>
        <AlertDialog.Content>
            <AlertDialog.Header>
                <AlertDialog.Title class="flex items-center gap-2 text-destructive">
                    <AlertTriangle class="h-5 w-5" />
                    确定要恢复数据吗？
                </AlertDialog.Title>
                <AlertDialog.Description class="space-y-3 pt-2">
                    <p>
                        您选择了备份文件：<span class="font-mono bg-muted px-1 py-0.5 rounded text-foreground">{selectedFile?.name}</span>
                    </p>
                    <p>
                        此操作将 <strong class="text-destructive">永久删除</strong> 当前系统中的所有数据，并使用备份文件进行覆盖。
                    </p>
                    <p class="font-bold">
                        操作无法撤销。请再次确认。
                    </p>
                </AlertDialog.Description>
            </AlertDialog.Header>
            <AlertDialog.Footer>
                <AlertDialog.Cancel onclick={handleRestoreCancel}>取消</AlertDialog.Cancel>
                <AlertDialog.Action onclick={handleRestoreConfirm} class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
                    确认恢复
                </AlertDialog.Action>
            </AlertDialog.Footer>
        </AlertDialog.Content>
    </AlertDialog.Root>

    <!-- Success Dialog -->
    <AlertDialog.Root open={showPostRestoreDialog}>
        <AlertDialog.Content>
            <AlertDialog.Header>
                <AlertDialog.Title class="flex items-center gap-2 text-green-600">
                    <DatabaseBackup class="h-5 w-5" />
                    数据恢复成功
                </AlertDialog.Title>
                <AlertDialog.Description class="space-y-4 pt-2 text-base">
                    <p>
                        您的数据已成功导入。
                    </p>
                    <div class="bg-muted p-4 rounded-md border text-sm !text-left">
                        <p class="mb-2 font-medium">请注意：</p>
                        <ul class="list-disc ml-4 space-y-1">
                            <li>您需要使用备份文件中的用户名进行登录：<br/><strong class="text-primary">{restoredUsername}</strong></li>
                            <li>为了确保数据库连接正常，<strong class="text-destructive">请务必手动重启服务</strong></li>
                        </ul>
                    </div>
                </AlertDialog.Description>
            </AlertDialog.Header>
            <AlertDialog.Footer>
                <Button onclick={handleLogoutAndRestart} variant="destructive" class="w-full">
                    需要手动重启服务，确定（直接登录不行）
                </Button>
            </AlertDialog.Footer>
        </AlertDialog.Content>
    </AlertDialog.Root>
</div>

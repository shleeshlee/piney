<script lang="ts">
    import { onMount } from "svelte";
    import { breadcrumbs } from "$lib/stores/breadcrumb";
    import { Button } from "$lib/components/ui/button";
    import { Badge } from "$lib/components/ui/badge";
    import * as AlertDialog from "$lib/components/ui/alert-dialog";
    import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
    import { SlidersHorizontal, Upload, Trash2, Download, FileJson, Regex } from "lucide-svelte";
    import { toast } from "svelte-sonner";
    import { goto } from "$app/navigation";

    import { API_BASE } from "$lib/api";
    import { downloadFile } from "$lib/utils/download";

    let items: any[] = $state([]);
    let loading = $state(true);
    let deleteDialogOpen = $state(false);
    let itemToDelete: string | null = $state(null);
    let fileInput: HTMLInputElement;

    onMount(async () => {
        breadcrumbs.set([{ label: "预设", href: "/presets" }]);
        await loadItems();
    });

    async function loadItems() {
        loading = true;
        try {
            const token = localStorage.getItem("auth_token");
            const headers: Record<string, string> = token ? { Authorization: `Bearer ${token}` } : {};
            const res = await fetch(`${API_BASE}/api/presets`, { headers });
            if (!res.ok) throw new Error("加载预设列表失败");
            items = await res.json();
        } catch (e) {
            console.error(e);
            toast.error("加载失败", { description: String(e) });
        } finally {
            loading = false;
        }
    }

    function triggerImport() {
        fileInput?.click();
    }

    async function handleImport(event: Event) {
        const input = event.target as HTMLInputElement;
        const files = input.files;
        if (!files || files.length === 0) return;

        const formData = new FormData();
        for (const file of files) {
            formData.append("file", file);
        }

        try {
            const token = localStorage.getItem("auth_token");
            const headers: Record<string, string> = token ? { Authorization: `Bearer ${token}` } : {};
            const res = await fetch(`${API_BASE}/api/presets/import`, {
                method: "POST",
                headers,
                body: formData,
            });
            if (!res.ok) throw new Error("导入失败");
            const results = await res.json();

            let successCount = 0;
            let errorMessages: string[] = [];
            for (const r of results) {
                if (r.status === "success") {
                    successCount++;
                } else {
                    errorMessages.push(`${r.file_name}: ${r.reason}`);
                }
            }

            if (successCount > 0) {
                toast.success(`成功导入 ${successCount} 个预设`);
            }
            if (errorMessages.length > 0) {
                toast.error("部分文件导入失败", { description: errorMessages.join("\n") });
            }

            await loadItems();
        } catch (e) {
            console.error(e);
            toast.error("导入失败", { description: String(e) });
        } finally {
            input.value = "";
        }
    }

    async function confirmDelete() {
        if (!itemToDelete) return;
        deleteDialogOpen = false;
        try {
            const token = localStorage.getItem("auth_token");
            const headers: Record<string, string> = token ? { Authorization: `Bearer ${token}` } : {};
            const res = await fetch(`${API_BASE}/api/presets/${itemToDelete}`, {
                method: "DELETE",
                headers,
            });
            if (!res.ok) throw new Error("删除失败");
            toast.success("删除成功");
            await loadItems();
        } catch (e) {
            console.error(e);
            toast.error("删除失败", { description: String(e) });
        } finally {
            itemToDelete = null;
        }
    }


</script>

<!-- 隐藏的文件输入 -->
<input
    bind:this={fileInput}
    type="file"
    accept=".json"
    multiple
    class="hidden"
    onchange={handleImport}
/>

<div class="container mx-auto py-6 space-y-8 max-w-7xl">
    <!-- 顶部 -->
    <div class="flex items-center justify-between">
        <div class="space-y-1">
            <h1 class="text-2xl font-bold tracking-tight">预设</h1>
            <p class="text-muted-foreground">管理您的聊天补全预设及其配套正则</p>
        </div>
        <div class="flex items-center gap-2">
            <Button onclick={triggerImport} class="gap-2">
                <Upload class="h-4 w-4" /> 导入预设
            </Button>
        </div>
    </div>

    <!-- 卡片网格 -->
    {#if loading}
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
            {#each Array(4) as _}
                <div class="h-36 rounded-xl bg-muted/50 animate-pulse"></div>
            {/each}
        </div>
    {:else if items.length > 0}
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 sm:gap-6">
            {#each items as item (item.id)}
                <div
                    class="group relative rounded-xl border bg-card text-card-foreground shadow-sm transition-all hover:border-primary/50 hover:shadow-md cursor-pointer flex flex-col"
                    role="button"
                    tabindex="0"
                    onkeydown={(e) => {
                        if (e.key === "Enter") goto(`/presets/${item.id}`);
                    }}
                    onclick={() => goto(`/presets/${item.id}`)}
                >
                    <!-- Regex Badge -->
                    {#if item.has_regex}
                        <div class="absolute top-3 right-3 z-10">
                            <Badge variant="secondary" class="bg-primary/10 text-primary border-primary/20 text-[10px] px-1.5 py-0.5 font-bold font-mono">
                                R
                            </Badge>
                        </div>
                    {/if}

                    <div class="p-4 flex flex-col h-full space-y-3">
                        <!-- 标题和备注 -->
                        <div class="flex-1 min-w-0 space-y-1.5 pt-0.5">
                            <h3 class="font-semibold text-base leading-tight break-words pr-8 line-clamp-2" title={item.title}>
                                {item.title}
                            </h3>
                            {#if item.user_note}
                                <p class="text-xs text-muted-foreground line-clamp-3 break-words leading-relaxed">
                                    {item.user_note}
                                </p>
                            {:else}
                                <p class="text-xs text-muted-foreground/50 italic">
                                    暂无备注
                                </p>
                            {/if}
                        </div>

                        <!-- 底部操作栏 -->
                        <div
                            class="flex items-center justify-between pt-2 mt-auto gap-2 border-t border-dashed/40"
                            role="none"
                            onclick={(e) => e.stopPropagation()}
                            onkeydown={(e) => e.stopPropagation()}
                        >
                            <div class="h-6 flex items-center">
                                <span class="text-[10px] text-muted-foreground/60 font-mono tracking-tight translate-y-[1px]">
                                    版本 v{item.version || '1.0.0'}
                                </span>
                            </div>
                            
                            <div class="flex items-center gap-0.5 -mr-1">
                                <!-- 导出下拉 -->
                                <DropdownMenu.Root>
                                    <DropdownMenu.Trigger>
                                        <Button variant="ghost" size="icon" class="h-6 w-6 text-muted-foreground hover:text-foreground">
                                            <Download class="h-3.5 w-3.5" />
                                        </Button>
                                    </DropdownMenu.Trigger>
                                    <DropdownMenu.Content align="end">
                                        <DropdownMenu.Item
                                            onclick={() => {
                                                const token = localStorage.getItem("auth_token");
                                                downloadFile({
                                                    url: `${API_BASE}/api/presets/${item.id}/export`,
                                                    filename: `${item.title}.json`,
                                                    fetchOptions: token ? { headers: { Authorization: `Bearer ${token}` } } : undefined,
                                                });
                                            }}
                                        >
                                            <Download class="h-4 w-4 mr-2" />
                                            导出预设（包含正则）
                                        </DropdownMenu.Item>
                                        <DropdownMenu.Item
                                            onclick={() => {
                                                const token = localStorage.getItem("auth_token");
                                                downloadFile({
                                                    url: `${API_BASE}/api/presets/${item.id}/export-regex`,
                                                    filename: `${item.title}_regex.json`,
                                                    fetchOptions: token ? { headers: { Authorization: `Bearer ${token}` } } : undefined,
                                                });
                                            }}
                                        >
                                            <FileJson class="h-4 w-4 mr-2" />
                                            仅导出正则包
                                        </DropdownMenu.Item>
                                    </DropdownMenu.Content>
                                </DropdownMenu.Root>

                                <!-- 删除 -->
                                <Button
                                    variant="ghost"
                                    size="icon"
                                    class="h-6 w-6 text-muted-foreground hover:text-destructive"
                                    onclick={() => {
                                        itemToDelete = item.id;
                                        deleteDialogOpen = true;
                                    }}
                                >
                                    <Trash2 class="h-3.5 w-3.5" />
                                </Button>
                            </div>
                        </div>
                    </div>
                </div>
            {/each}
        </div>
    {:else}
        <div class="text-center py-20">
            <div class="mx-auto w-12 h-12 rounded-full bg-muted/50 flex items-center justify-center mb-4">
                <SlidersHorizontal class="h-6 w-6 text-muted-foreground" />
            </div>
            <h3 class="text-lg font-medium">暂无预设</h3>
            <p class="text-muted-foreground mt-1">
                点击上方"导入预设"添加您的第一个预设
            </p>
        </div>
    {/if}

    <!-- 删除确认弹窗 -->
    <AlertDialog.Root bind:open={deleteDialogOpen}>
        <AlertDialog.Content>
            <AlertDialog.Header>
                <AlertDialog.Title>确定要删除吗？</AlertDialog.Title>
                <AlertDialog.Description>
                    此操作将永久删除该预设及其配套正则，且无法恢复。
                </AlertDialog.Description>
            </AlertDialog.Header>
            <AlertDialog.Footer>
                <AlertDialog.Cancel onclick={() => (itemToDelete = null)}>取消</AlertDialog.Cancel>
                <AlertDialog.Action
                    class="bg-destructive !text-destructive-foreground hover:bg-destructive/90"
                    onclick={confirmDelete}
                >
                    确认删除
                </AlertDialog.Action>
            </AlertDialog.Footer>
        </AlertDialog.Content>
    </AlertDialog.Root>
</div>

<script lang="ts">
    import { Button } from "$lib/components/ui/button";
    import * as Dialog from "$lib/components/ui/dialog";
    import { Checkbox } from "$lib/components/ui/checkbox";
    import { Label } from "$lib/components/ui/label";
    import { Input } from "$lib/components/ui/input";
    import { toast } from "svelte-sonner";
    import { api } from "$lib/api";
    import { Loader2, Search, ArrowRight, ArrowLeft, Check, Plus } from "lucide-svelte";
    import { page } from "$app/stores";
    import * as Drawer from "$lib/components/ui/drawer";

    let { currentPresetId, currentPresetTitle, currentItems, onSave, open = $bindable(false), mode = $bindable("to_other") } = $props();

    // 状态机步骤
    type Step = "SELECT_PRESET" | "SELECT_ITEMS" | "SELECT_POSITION";
    let currentStep: Step = $state("SELECT_PRESET");

    // 数据
    let presets: any[] = $state([]);
    let filteredPresets: any[] = $derived(
        presets.filter(p => p.title.toLowerCase().includes(searchQuery.toLowerCase()))
    );
    let selectedPresetId: string | null = $state(null);
    let selectedPresetDetails: any = $state(null);
    let selectedItemIds: Set<string> = $state(new Set());
    
    // UI 状态
    let loading = $state(false);
    let searchQuery = $state("");
    let targetItems: any[] = $state([]); // 目标预设的条目列表（用于选择位置）
    let sourceItems: any[] = $state([]); // 来源预设的条目列表（用于选择条目）
    
    // 插入位置选择
    let positionSelectionOpen = $state(false);
    let targetInsertItem: any = $state(null); // 选中的目标锚点条目

    // 初始化重置
    $effect(() => {
        if (open) {
            reset();
            loadPresets();
            // 根据模式决定第一步
            if (mode === "to_other") {
                // 缝合到其他：当前页是来源，直接去选目标预设（但条目得先选）
                // 修正逻辑：先选条目（当前页），再选目标预设，再选位置
                sourceItems = currentItems;
                currentStep = "SELECT_ITEMS";
            } else {
                // 从其他缝合：先选来源预设，再选条目，再选位置（当前页）
                currentStep = "SELECT_PRESET";
            }
        }
    });

    function reset() {
        currentStep = "SELECT_PRESET";
        selectedPresetId = null;
        selectedPresetDetails = null;
        selectedItemIds = new Set();
        searchQuery = "";
        loading = false;
        targetItems = [];
        sourceItems = [];
        positionSelectionOpen = false;
        targetInsertItem = null;
    }

    async function loadPresets() {
        try {
            loading = true;
            loading = true;
            const res = await api.get<any[]>("/presets");
            if (res.success && Array.isArray(res.data)) {
                // 排除当前预设
                presets = res.data.filter((p: any) => p.id !== currentPresetId);
            }
        } catch (e) {
            toast.error("加载预设列表失败");
        } finally {
            loading = false; // 只有在最后才结束 loading
        }
    }

    async function loadPresetDetails(id: string) {
        try {
            loading = true;
            loading = true;
            const res = await api.get<any>(`/presets/${id}`);
            if (res.success) {
                const item = res.data;
                if (item.data && typeof item.data === 'string') {
                    try {
                        item.data = JSON.parse(item.data);
                    } catch (e) {
                        console.error("Failed to parse preset data", e);
                        item.data = {};
                    }
                }
                return item;
            }
        } catch (e) {
            toast.error("加载预设详情失败");
            return null;
        } finally {
            loading = false;
        }
    }

    // --- 步骤逻辑 ---

    async function handlePresetSelect(preset: any) {
        selectedPresetId = preset.id;
        const details = await loadPresetDetails(preset.id);
        if (!details) return;
        selectedPresetDetails = details;

        if (mode === "from_other") {
            // 来源选好了，去选条目 (全部条目)
            sourceItems = parseItemsFromDetails(details);
            currentStep = "SELECT_ITEMS";
        } else {
            // 目标选好了（且条目在第一步已选好），去选位置 (仅 active 条目)
            const allItems = parseItemsFromDetails(details);
            targetItems = allItems.filter(p => p._inOrder);
            currentStep = "SELECT_POSITION";
        }
    }

    function handleItemsConfirm() {
        if (selectedItemIds.size === 0) {
            toast.error("请至少选择一个条目");
            return;
        }

        if (mode === "to_other") {
            // 条目选好了，去选目标预设
            currentStep = "SELECT_PRESET";
        } else {
             targetItems = currentItems.filter((p: any) => p.enabled !== false && p._inOrder !== false);
            currentStep = "SELECT_POSITION";
        }
    }

    function toggleSelection(id: string) {
        if (selectedItemIds.has(id)) {
            selectedItemIds.delete(id);
        } else {
            selectedItemIds.add(id);
        }
        selectedItemIds = new Set(selectedItemIds); // trigger update
    }

    // 解析预设详情中的条目列表（复用主页面的逻辑简化版）
    function parseItemsFromDetails(data: any): any[] {
        let rawPrompts: any[] = [];

        if (data.data?.prompts && Array.isArray(data.data.prompts)) {
             rawPrompts = [...data.data.prompts];
        } else if (data.data?.prompt_order) {
            // 只有 prompt_order 的情况（罕见但存在）
            // 实际上如果没有 prompts 数组，我们也无法获取内容，所以通常假设有
            return [];
        }

        // 处理 magic prompt_order 元素
        let order = data.data.prompt_order || data.prompt_order;
        const magicIndex = rawPrompts.findIndex((p: any) => p.prompt_order && Array.isArray(p.prompt_order));
        if (magicIndex !== -1) {
            if (!order) order = rawPrompts[magicIndex].prompt_order;
            rawPrompts.splice(magicIndex, 1);
        }

        // 展平 order（支持多 character_id 嵌套格式）
        let flatOrder: any[] = [];
        if (order) {
             if (order.length > 0 && order[0]?.order && Array.isArray(order[0].order)) {
                 // 嵌套格式：合并所有 character_id 的 order，以条目最多的为主排序
                 let primaryEntry = order[0];
                 for (const entry of order) {
                     if (entry.order?.length > (primaryEntry.order?.length || 0)) {
                         primaryEntry = entry;
                     }
                 }
                 const mergedMap = new Map<string, any>();
                 const mergedList: any[] = [];
                 for (const item of primaryEntry.order) {
                     if (!item.identifier) continue;
                     mergedMap.set(item.identifier, { ...item });
                     mergedList.push(mergedMap.get(item.identifier));
                 }
                 for (const entry of order) {
                     if (entry === primaryEntry) continue;
                     if (entry.order && Array.isArray(entry.order)) {
                         for (const item of entry.order) {
                             if (!item.identifier) continue;
                             if (mergedMap.has(item.identifier)) {
                                 mergedMap.get(item.identifier).enabled = item.enabled;
                             } else {
                                 mergedMap.set(item.identifier, { ...item });
                                 mergedList.push(mergedMap.get(item.identifier));
                             }
                         }
                     }
                 }
                 flatOrder = mergedList;
             } else {
                 flatOrder = order;
             }
        }

        // 排序
        const promptMap = new Map(rawPrompts.map((p: any) => [p.identifier, p]));
        const ordered: any[] = [];
        const visited = new Set();
        
        for (const o of flatOrder) {
            if (!o.identifier) continue;
            const p = promptMap.get(o.identifier);
            if (p) {
                p._inOrder = true; // 标记为在 order 中
                // 不覆盖 p.enabled，enabled 是条目自身的开关，与 _inOrder 无关
                ordered.push(p);
                visited.add(o.identifier);
            }
        }
        for (const p of rawPrompts) {
            if (p.identifier && !visited.has(p.identifier)) {
                p._inOrder = false; // 标记不在 order 中
                // 不覆盖 p.enabled，enabled 是条目自身的开关
                ordered.push(p);
            }
        }
        return ordered;
    }

    // --- 最终执行 ---

    async function executeStitch(position: "before" | "after") {
        if (!targetInsertItem && targetItems.length > 0) return; // 必须选中锚点
        
        loading = true;
        try {
            // 1. 准备要插入的条目（深拷贝并重生成 ID）
            const itemsToInsert = sourceItems
                .filter(p => selectedItemIds.has(p.identifier))
                .map(p => {
                    const { _inOrder, ...rest } = p;
                    return {
                        ...rest,
                        identifier: crypto.randomUUID() // 重新生成 ID 防冲突
                    };
                });

            // 2. 确定插入索引
            let insertIndex = -1;
            if (targetItems.length === 0) {
                 insertIndex = 0;
            } else {
                const anchorIndex = targetItems.findIndex(p => p.identifier === targetInsertItem.identifier);
                if (anchorIndex === -1) {
                    toast.error("定位锚点失败"); 
                    loading = false;
                    return;
                }
                insertIndex = position === "before" ? anchorIndex : anchorIndex + 1;
            }
            
            if (mode === "to_other") {
                await stitchToRemote(itemsToInsert, insertIndex);
            } else {
                await stitchToLocal(itemsToInsert, insertIndex);
            }
            
            positionSelectionOpen = false;
            open = false;
        } catch (e: any) {
            console.error(e);
            toast.error(e.message || "缝合失败");
        } finally {
            loading = false;
        }
    }

    async function stitchToRemote(newItems: any[], insertIndex: number) {

        // 1. 更新 prompts 列表：插入到锚点对应位置
        let rawPrompts = selectedPresetDetails.data?.prompts || [];
        // 确保是数组
        if (!Array.isArray(rawPrompts)) rawPrompts = [];
        
        // 找到锚点在 rawPrompts 中的位置，将新条目插入到同样的位置
        const updatedPrompts = [...rawPrompts];
        if (targetInsertItem) {
            const anchorIdx = updatedPrompts.findIndex(p => p.identifier === targetInsertItem.identifier);
            if (anchorIdx !== -1) {
                // insertIndex 是基于 targetItems（仅 active 项）的，
                // 这里用锚点位置来决定在 prompts 中的插入点
                const promptInsertIdx = insertIndex > targetItems.indexOf(targetInsertItem) 
                    ? anchorIdx + 1  // 插入到锚点后面
                    : anchorIdx;     // 插入到锚点前面
                updatedPrompts.splice(promptInsertIdx, 0, ...newItems);
            } else {
                updatedPrompts.push(...newItems);
            }
        } else {
            // 空列表或无锚点，直接追加
            updatedPrompts.push(...newItems);
        }
        
        // 2. 更新 prompt_order
        // 我们需要基于 flatOrder 插入引用，然后重组回 nested 结构
        let order = selectedPresetDetails.data?.prompt_order || selectedPresetDetails.prompt_order;
        
        let flatOrder: any[] = [];
        let isNested = false;
        let nestedOrderEntries: any[] = []; // 保存完整嵌套结构
        let primaryCharacterId: number | undefined;

        if (order && Array.isArray(order)) {
             if (order.length > 0 && order[0]?.order && Array.isArray(order[0].order)) {
                 isNested = true;
                 nestedOrderEntries = JSON.parse(JSON.stringify(order)); // 深拷贝
                 // 找到主排序组
                 let primaryEntry = order[0];
                 for (const entry of order) {
                     if (entry.order?.length > (primaryEntry.order?.length || 0)) {
                         primaryEntry = entry;
                     }
                 }
                 primaryCharacterId = primaryEntry.character_id;
                 // 合并所有 order
                 const mergedMap = new Map<string, any>();
                 const mergedList: any[] = [];
                 for (const item of primaryEntry.order) {
                     if (!item.identifier) continue;
                     mergedMap.set(item.identifier, { ...item });
                     mergedList.push(mergedMap.get(item.identifier));
                 }
                 for (const entry of order) {
                     if (entry === primaryEntry) continue;
                     if (entry.order && Array.isArray(entry.order)) {
                         for (const item of entry.order) {
                             if (!item.identifier) continue;
                             if (mergedMap.has(item.identifier)) {
                                 mergedMap.get(item.identifier).enabled = item.enabled;
                             } else {
                                 mergedMap.set(item.identifier, { ...item });
                                 mergedList.push(mergedMap.get(item.identifier));
                             }
                         }
                     }
                 }
                 flatOrder = mergedList;
             } else {
                  flatOrder = [...order];
             }
        }
        
        // 在 flatOrder 中插入引用
        const currentVisualOrder = [...targetItems];
        
        // 新条目保留原始 enabled 状态
        const preparedNewItems = newItems.map(p => ({
            ...p,
            enabled: p.enabled !== false,
        }));

        currentVisualOrder.splice(insertIndex, 0, ...preparedNewItems);
        
        // 映射回 prompt_order 格式，保留原始 enabled 状态
        const newFlatOrder = currentVisualOrder.map(p => ({
            identifier: p.identifier,
            enabled: p.enabled !== false,
        }));
        
        // 还原嵌套结构
        let finalOrder: any;
        if (isNested && nestedOrderEntries.length > 0) {
            // 保留原始的多 character_id 结构
            const enabledMap = new Map<string, boolean>();
            for (const item of newFlatOrder) {
                enabledMap.set(item.identifier, item.enabled);
            }
            finalOrder = nestedOrderEntries.map((entry: any) => {
                if (entry.character_id === primaryCharacterId) {
                    // 主排序组：替换为新顺序
                    return { character_id: entry.character_id, order: newFlatOrder };
                } else {
                    // 其他组：保留原始结构，仅同步 enabled
                    return {
                        character_id: entry.character_id,
                        order: (entry.order || []).map((item: any) => ({
                            ...item,
                            enabled: enabledMap.has(item.identifier)
                                ? enabledMap.get(item.identifier)
                                : item.enabled,
                        })),
                    };
                }
            });
        } else {
            finalOrder = newFlatOrder;
        }
            
        // 构造 Patch Payload（清理内部标记 _inOrder）
        const cleanedPrompts = updatedPrompts.map((p: any) => {
            const { _inOrder, ...rest } = p;
            return rest;
        });

        const payload = {
            data: {
                ...selectedPresetDetails.data,
                prompts: cleanedPrompts,
                prompt_order: finalOrder
            }
        };

        const res = await api.patch(`/presets/${selectedPresetId}`, payload);
        
        if (!res.success) throw new Error(res.error || "保存远程预设失败");
        toast.success("成功缝合到目标预设！");
    }

    async function stitchToLocal(newItems: any[], insertIndex: number) {
        await onSave({ newItems, insertIndex }); 
        toast.success("成功缝合到本地并保存！");
    }

</script>

<Dialog.Root bind:open={open}>
    <Dialog.Content class="max-w-2xl max-h-[90vh] flex flex-col p-0 gap-0">
        <!-- Header -->
        <div class="p-6 pb-2 border-b shrink-0">
            <Dialog.Title class="text-xl font-bold">
                {mode === "to_other" ? "将条目缝合到其他预设" : "从其他预设缝合条目"}
            </Dialog.Title>
            <Dialog.Description class="mt-1">
                {#if currentStep === "SELECT_PRESET"}
                    请选择{mode === "to_other" ? "目标" : "来源"}预设
                {:else if currentStep === "SELECT_ITEMS"}
                    请选择要复制的条目（{selectedItemIds.size}）
                {:else if currentStep === "SELECT_POSITION"}
                    点击列表条目以选择插入位置
                {/if}
            </Dialog.Description>
        </div>

        <!-- Body -->
        <div class="flex-1 overflow-hidden min-h-[400px] flex flex-col relative">
            {#if loading}
                <div class="absolute inset-0 z-10 bg-background/50 backdrop-blur-sm flex items-center justify-center">
                    <Loader2 class="w-8 h-8 animate-spin text-primary" />
                </div>
            {/if}

            <!-- 步骤 1: 选择预设 -->
            {#if currentStep === "SELECT_PRESET"}
                <div class="p-4 border-b">
                    <div class="relative">
                        <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                        <Input class="pl-9" placeholder="搜索预设..." bind:value={searchQuery} />
                    </div>
                </div>
                <div class="flex-1 overflow-y-auto min-h-0">
                    <div class="p-2 space-y-1">
                        {#each filteredPresets as preset (preset.id)}
                            <button
                                class="w-full text-left px-4 py-3 rounded-md hover:bg-accent flex items-center justify-between group transition-colors"
                                onclick={() => handlePresetSelect(preset)}
                            >
                                <div>
                                    <div class="font-medium">{preset.title}</div>
                                    <div class="text-sm text-muted-foreground/0 group-hover:text-muted-foreground/100 transition-colors">
                                        点击选择
                                    </div>
                                </div>
                                <ArrowRight class="w-4 h-4 opacity-0 group-hover:opacity-100" />
                            </button>
                        {/each}
                        {#if filteredPresets.length === 0}
                            <div class="text-center py-8 text-muted-foreground">未找到预设</div>
                        {/if}
                    </div>
                </div>
            {/if}

            <!-- 步骤 2: 选择条目 -->
            {#if currentStep === "SELECT_ITEMS"}
                <div class="flex-1 overflow-y-auto min-h-0">
                    <div class="p-2">
                         <div class="px-4 py-2 text-sm text-muted-foreground bg-muted/30 rounded mb-2">
                             来自：{mode === "to_other" ? currentPresetTitle : selectedPresetDetails?.title}
                         </div>
                         <div class="space-y-1">
                             {#each sourceItems as item (item.identifier)}
                                <button 
                                    class="w-full text-left flex items-center space-x-3 px-4 py-2 hover:bg-accent rounded-md cursor-pointer transition-colors border-0 bg-transparent"
                                    onclick={() => toggleSelection(item.identifier)}
                                >
                                    <Checkbox 
                                        id={item.identifier}
                                        checked={selectedItemIds.has(item.identifier)}
                                        class="pointer-events-none"
                                    />
                                    <div class="grid gap-1.5 leading-none pointer-events-none">
                                        <Label
                                            for={item.identifier}
                                            class="text-sm font-medium leading-none cursor-pointer"
                                        >
                                            {item.name || "未命名条目"}
                                        </Label>
                                        {#if item.role}
                                            <p class="text-xs text-muted-foreground">
                                                Role: {item.role}
                                            </p>
                                        {/if}
                                    </div>
                                </button>
                             {/each}
                         </div>
                    </div>
                </div>
                <div class="p-4 border-t flex justify-end gap-2 shrink-0 bg-background">
                    <Button variant="outline" onclick={reset}>取消</Button>
                    <Button onclick={handleItemsConfirm} disabled={selectedItemIds.size === 0}>
                        下一步 ({selectedItemIds.size}) <ArrowRight class="ml-2 w-4 h-4" />
                    </Button>
                </div>
            {/if}

            <!-- 步骤 3: 选择位置 -->
            {#if currentStep === "SELECT_POSITION"}
                 <div class="flex-1 overflow-y-auto min-h-0">
                    <div class="p-2">
                        <div class="px-4 py-2 text-sm text-muted-foreground bg-muted/30 rounded mb-2">
                             目标：{mode === "to_other" ? selectedPresetDetails?.title : currentPresetTitle}
                         </div>
                        <div class="space-y-1">
                            {#each targetItems as item (item.identifier)}
                                <button
                                    class="w-full text-left px-4 py-3 rounded-md hover:bg-accent border border-transparent hover:border-primary/20 transition-all flex items-center justify-between group"
                                    onclick={() => {
                                        targetInsertItem = item;
                                        positionSelectionOpen = true;
                                    }}
                                >
                                    <span class="font-medium text-sm">{item.name || "未命名条目"}</span>
                                    <span class="text-xs text-primary opacity-0 group-hover:opacity-100 bg-primary/10 px-2 py-1 rounded">插入此处</span>
                                </button>
                            {/each}
                            {#if targetItems.length === 0}
                                <button
                                    class="w-full text-center py-8 rounded-md hover:bg-accent border border-dashed border-muted-foreground/20 text-muted-foreground"
                                    onclick={() => {
                                        targetInsertItem = null; // 空列表
                                        executeStitch("after"); // 无所谓前后
                                    }}
                                >
                                    列表为空，点击直接插入
                                </button>
                            {/if}
                        </div>
                    </div>
                 </div>
                 <div class="p-4 border-t flex justify-between gap-2 shrink-0 bg-background">
                    <Button variant="ghost" onclick={() => {
                        // Back logic
                        if (mode === "to_other") currentStep = "SELECT_PRESET";
                        else currentStep = "SELECT_ITEMS";
                    }}>
                        <ArrowLeft class="mr-2 w-4 h-4" /> 上一步
                    </Button>
                    <div class="text-xs text-muted-foreground flex items-center">
                        点击列表项弹出插入菜单
                    </div>
                </div>
            {/if}
        </div>
    </Dialog.Content>
</Dialog.Root>

<!-- 底部操作栏（用于选择插入位置） -->
<Drawer.Root bind:open={positionSelectionOpen}>
    <Drawer.Content>
        <div class="mx-auto w-full max-w-sm">
            <Drawer.Header>
                <Drawer.Title>插入位置</Drawer.Title>
                <Drawer.Description>
                    相对于 "{targetInsertItem?.name || '选中条目'}"
                </Drawer.Description>
            </Drawer.Header>
            <div class="p-4 pb-8 space-y-4">
                <Button class="w-full" variant="secondary" onclick={() => executeStitch("before")}>
                     <ArrowRight class="mr-2 w-4 h-4 -rotate-90" /> 插入到上方
                </Button>
                <Button class="w-full" onclick={() => executeStitch("after")}>
                     <ArrowRight class="mr-2 w-4 h-4 rotate-90" /> 插入到下方
                </Button>
            </div>
        </div>
    </Drawer.Content>
</Drawer.Root>

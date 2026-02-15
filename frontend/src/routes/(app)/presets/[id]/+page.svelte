<script lang="ts">
    import { page } from "$app/stores";
    import { onMount } from "svelte";
    import { breadcrumbs } from "$lib/stores/breadcrumb";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import { Textarea } from "$lib/components/ui/textarea";
    import DirtyInput from "$lib/components/common/DirtyInput.svelte";
    import DirtyTextarea from "$lib/components/common/DirtyTextarea.svelte";
    import { Switch } from "$lib/components/ui/switch";
    import { Badge } from "$lib/components/ui/badge";
    import * as Tabs from "$lib/components/ui/tabs";
    import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
    import { ScrollArea } from "$lib/components/ui/scroll-area";
    import {
        Save, Loader2, ArrowLeft, Download, FileJson,
        ChevronDown, Trash2, Plus, GripVertical, Regex as RegexIcon, Upload, ArrowUpDown, X,
        Combine, ArrowRight // Added
    } from "lucide-svelte";
    import { toast } from "svelte-sonner";
    import { goto } from "$app/navigation";
    import PipiStudyTab from "$lib/components/presets/PipiStudyTab.svelte";
    import { useUnsavedChanges } from "$lib/hooks/use-unsaved-changes.svelte";
    import UnsavedGuard from "$lib/components/common/UnsavedGuard.svelte";
    import StitchModal from "$lib/components/presets/StitchModal.svelte"; // Added
    import { cn } from "$lib/utils";
    import {
        dndzone,
        type DndEvent,
        SHADOW_ITEM_MARKER_PROPERTY_NAME,
        TRIGGERS,
    } from "svelte-dnd-action";
    import { flip } from "svelte/animate";

    import { API_BASE } from "$lib/api";
    import { downloadFile } from "$lib/utils/download";

    let id = $page.params.id;
    let loading = $state(true);
    let saving = $state(false);
    let item: any = $state(null);

    // 缝合功能状态
    let stitchModalOpen = $state(false);
    let stitchMode = $state("to_other");

    // ... (rest of the code)

    async function handleStitchSave({ newItems, insertIndex }: { newItems: any[], insertIndex: number }) {
        // 将新条目插入到 enabledPrompts (通常缝合进来的认为是 enabled?)
        // 用户可能是在“全部”列表里选的位置，但 StitchModal 里我们传的是 currentItems (allPrompts?)
        // 不，StitchModal 里传的是 currentItems -> parseItemsFromDetails -> ordered (which is basically enabledPrompts + disabledPrompts ordered)
        // 但是 position selection 用的是 targetItems (currentItems).
        
        // 简单起见，我们只能插入到 enabledPrompts 中 (因为 disabled 不显示在排序列表里)
        // 如果 insertIndex 越界，就 append
        
        const newEnabled = [...enabledPrompts];
        if (insertIndex >= newEnabled.length) {
            newEnabled.push(...newItems);
        } else {
            newEnabled.splice(insertIndex, 0, ...newItems);
        }
        
        // 标记为 enabled 并 inOrder
        newItems.forEach(p => {
            p.enabled = true;
            p._inOrder = true;
        });
        
        enabledPrompts = newEnabled;
        onPromptChange(); // 标记 dirty
        
        // 立即保存
        await save();
    }

    // 预设数据
    let presetData: any = $state({});
    // 拆分后的 prompts 列表
    let enabledPrompts: any[] = $state([]);
    let disabledPrompts: any[] = $state([]);
    
    let regexData: any[] = $state([]);
    let presetTitle = $state("");
    let userNote = $state("");
    let version = $state("1.0.0");
    let pipiStudy = $state("");

    // 脏状态
    let isDirty = $state(false);
    let lastSaved = $state(0);
    let originalState = "";

    // 未保存更改守卫
    const unsaved = useUnsavedChanges(() => isDirty);

    // 条目展开状态
    let openPrompts: Record<string, boolean> = $state({});

    // 当前 Tab
    let activeTab = $state("prompts");

    // --- 正则相关状态 ---
    let openScripts: Record<string, boolean> = $state({});
    let originalRegex = "";
    const FLIP_DURATION_MS = 200;
    const TOUCH_DELAY_MS = 400;
    let isDragging = $state(false);
    let dndItems: any[] = $state([]);
    
    let fileInput: HTMLInputElement | undefined = $state();

    function generateUUID() {
        return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
            var r = Math.random() * 16 | 0, v = c == 'x' ? r : (r & 0x3 | 0x8);
            return v.toString(16);
        });
    }

    onMount(async () => {
        await loadItem();
    });

    async function loadItem() {
        loading = true;
        try {
            const token = localStorage.getItem("auth_token");
            const res = await fetch(`${API_BASE}/api/presets/${id}`, {
                headers: token ? { Authorization: `Bearer ${token}` } : {},
            });
            if (!res.ok) throw new Error("加载失败");
            item = await res.json();
            presetTitle = item.title;
            userNote = item.user_note || "";
            pipiStudy = item.pipi_study || "";

            breadcrumbs.set([
                { label: "预设", href: "/presets" },
                { label: item.title },
            ]);

            // 解析 data JSON
            try {
                presetData = JSON.parse(item.data);
            } catch (e) {
                console.error("JSON 解析失败", e);
                presetData = {};
            }

            // 处理 prompts 排序与分组
            let rawPrompts = presetData.prompts || [];
            // 检查是否有 prompt_order（通常在 extensions 或根级，如果 ST 导出格式有变，这里预留检查）
            // 如果 prompt_order 存在于 data 中：
            let order = presetData.prompt_order;
            
            // 如果 data 中没有，检查是否有 prompt_order 作为 prompts 数组的特殊元素（用户提到的情况）
            const magicOrderIndex = rawPrompts.findIndex((p: any) => p.prompt_order && Array.isArray(p.prompt_order));
            if (magicOrderIndex !== -1) {
                order = rawPrompts[magicOrderIndex].prompt_order;
                // 从展示列表中移除
                rawPrompts.splice(magicOrderIndex, 1);
                // 存回 presetData 以供 analyzePreset 使用
                presetData.prompt_order = order;
            }

            if (order && Array.isArray(order)) {
                // 标准化 prompt_order 格式
                // SillyTavern 格式可能是嵌套的：[{character_id, order: [{identifier, enabled}]}, ...]
                // 一个 prompt_order 数组下可能有多个 character_id 的 order
                // 需要将所有 order 合并展平为 [{identifier, enabled}]
                let flatOrder = order;
                let _isNestedOrder = false;
                let _nestedOrderEntries: any[] = []; // 保存原始嵌套结构供 save 使用
                if (order.length > 0 && order[0]?.order && Array.isArray(order[0].order)) {
                    // 嵌套格式：合并所有 character_id 的 order 数组
                    _isNestedOrder = true;
                    _nestedOrderEntries = order; // 保存完整的嵌套结构
                    // 找到条目最多的 order 数组作为主排序依据
                    let primaryEntry = order[0];
                    for (const entry of order) {
                        if (entry.order?.length > (primaryEntry.order?.length || 0)) {
                            primaryEntry = entry;
                        }
                    }
                    // 以主排序组为基础，合并其他组中不存在的条目
                    const mergedMap = new Map<string, any>();
                    const mergedList: any[] = [];
                    // 先添加主排序组的所有条目（确定排列顺序）
                    for (const item of primaryEntry.order) {
                        if (!item.identifier) continue;
                        const newItem = { ...item };
                        mergedMap.set(item.identifier, newItem);
                        mergedList.push(newItem);
                    }
                    // 再遍历其他组，更新 enabled 状态或追加新条目
                    for (const entry of order) {
                        if (entry === primaryEntry) continue;
                        if (entry.order && Array.isArray(entry.order)) {
                            for (const item of entry.order) {
                                if (!item.identifier) continue;
                                if (mergedMap.has(item.identifier)) {
                                    // 重复条目：只更新 enabled 状态，位置不变
                                    const existing = mergedMap.get(item.identifier);
                                    existing.enabled = item.enabled;
                                } else {
                                    // 新条目：追加到末尾
                                    const newItem = { ...item };
                                    mergedMap.set(item.identifier, newItem);
                                    mergedList.push(newItem);
                                }
                            }
                        }
                    }
                    flatOrder = mergedList;
                }
                // 保存格式信息供 save 使用
                presetData._isNestedOrder = _isNestedOrder;
                presetData._nestedOrderEntries = _nestedOrderEntries;

                // 根据 order 排序和设置 enabled
                const promptMap = new Map(rawPrompts.map((p: any) => [p.identifier, p]));
                const ordered: any[] = [];
                const visited = new Set();
                
                // prompt_order 中的条目
                for (const o of flatOrder) {
                    if (!o.identifier) continue;
                    const p = promptMap.get(o.identifier);
                    if (p) {
                        // 使用 order 中的 enabled 状态覆盖
                        if (typeof o.enabled === "boolean") {
                            (p as any).enabled = o.enabled;
                        }
                        (p as any)._inOrder = true; // 标记为在 prompt_order 中
                        ordered.push(p);
                        visited.add(o.identifier);
                    }
                }

                // 不在 prompt_order 中的条目：保留原始 enabled 值
                for (const p of rawPrompts) {
                    if (p.identifier && !visited.has(p.identifier)) {
                        // 不覆盖 p.enabled，保留条目自身的开关状态
                        p._inOrder = false; // 标记为不在 prompt_order 中
                        ordered.push(p);
                    } else if (!p.identifier) {
                        // 没有 identifier 的条目也放到未启用区
                        p._inOrder = false;
                        ordered.push(p);
                    }
                }
                rawPrompts = ordered;
            } else {
                // 没有 prompt_order，所有条目默认在 order 中
                for (const p of rawPrompts) {
                    p._inOrder = true;
                }
            }

            // 分组：按 _inOrder 分组
            // _inOrder=true（在 prompt_order 中）→ 主列表
            // _inOrder=false（不在 prompt_order 中）→ 未启用列表
            enabledPrompts = rawPrompts.filter((p: any) => p._inOrder !== false);
            disabledPrompts = rawPrompts.filter((p: any) => p._inOrder === false);

            // 解析 regex_data
            try {
                regexData = JSON.parse(item.regex_data);
                if (!Array.isArray(regexData)) regexData = [];
            } catch (e) {
                regexData = [];
            }

            // 确保正则有 id
            regexData = regexData.map((s: any) => ({
                ...s,
                id: s.id || generateUUID(),
            }));

            snapshotState();
            snapshotPrompts(); // 记录每个条目的初始值
        } catch (e) {
            toast.error("加载失败", { description: String(e) });
            goto("/presets");
        } finally {
            loading = false;
        }
    }

    function snapshotState() {
        originalState = JSON.stringify({
            title: presetTitle,
            userNote: userNote,
            version: version,
            // 比较时合并两个列表
            prompts: [...enabledPrompts, ...disabledPrompts],
            regexData: regexData,
        });
        originalRegex = JSON.stringify(regexData);
        // 记录标题和备注的初始值
        initialTitle = presetTitle;
        initialNote = userNote;
        initialVersion = version;
        snapshotRegex();
    }

    // 标题和备注的初始值（用于字段级脏状态）
    let initialTitle = $state("");
    let initialNote = $state("");
    let initialVersion = $state("1.0.0");

    // 条目字段级脏状态跟踪
    let initialPromptSnapshot: Map<string, { name: string; content: string }> = new Map();

    function snapshotPrompts() {
        initialPromptSnapshot = new Map();
        const all = [...enabledPrompts, ...disabledPrompts];
        for (const p of all) {
            if (p.identifier) {
                initialPromptSnapshot.set(p.identifier, {
                    name: p.name || "",
                    content: p.content || "",
                });
            }
        }
    }

    function isFieldDirty(identifier: string, field: "name" | "content", currentValue: string): boolean {
        const snap = initialPromptSnapshot.get(identifier);
        if (!snap) return false; // 新增的条目不标记
        return (snap[field] || "") !== (currentValue || "");
    }

    // 删除条目
    function deletePrompt(prompt: any) {
        enabledPrompts = enabledPrompts.filter(p => p.identifier !== prompt.identifier);
        disabledPrompts = disabledPrompts.filter(p => p.identifier !== prompt.identifier);
        onPromptChange();
    }

    // 正则字段级脏状态跟踪
    let initialRegexSnapshot: Map<string, Record<string, any>> = new Map();

    function snapshotRegex() {
        initialRegexSnapshot = new Map();
        for (const s of regexData) {
            if (s.id) {
                initialRegexSnapshot.set(s.id, {
                    scriptName: s.scriptName || "",
                    findRegex: s.findRegex || "",
                    replaceString: s.replaceString || "",
                    minDepth: s.minDepth ?? "",
                    maxDepth: s.maxDepth ?? "",
                });
            }
        }
    }

    function isRegexFieldDirty(id: string, field: string, currentValue: any): boolean {
        const snap = initialRegexSnapshot.get(id);
        if (!snap) return false;
        const original = snap[field] ?? "";
        const current = currentValue ?? "";
        return String(original) !== String(current);
    }

    function checkDirty() {
        const current = JSON.stringify({
            title: presetTitle,
            userNote: userNote,
            version: version,
            prompts: [...enabledPrompts, ...disabledPrompts],
            regexData: regexData,
        });
        isDirty = current !== originalState;
    }

    // --- 保存 ---
    async function save() {
        saving = true;
        try {
            const token = localStorage.getItem("auth_token");

            // 同步正则到 data.extensions.regex_scripts
            if (!presetData.extensions) presetData.extensions = {};
            presetData.extensions.regex_scripts = regexData;

            // 重建 prompts 和 prompt_order
            // 1. 实际的 prompt 对象列表（合并）
            const allPrompts = [...enabledPrompts, ...disabledPrompts];
            
            // 2. 构建 prompt_order：仅包含 _inOrder 为 true 的条目
            const newOrder = allPrompts
                .filter(p => p._inOrder !== false)
                .map(p => ({
                    identifier: p.identifier,
                    enabled: p.enabled !== false,
                }));

            // 构造要保存的 prompts 数组（清理内部标记）
            const promptsToSave = allPrompts.map(p => {
                const { _inOrder, ...rest } = p;
                return rest;
            });

            // 根据原始格式构造 prompt_order
            let savedOrder: any;
            if (presetData._isNestedOrder && presetData._nestedOrderEntries?.length > 0) {
                // SillyTavern 嵌套格式：保留原始的多 character_id 结构
                // 将所有条目的 order 都更新为最新的 newOrder
                savedOrder = presetData._nestedOrderEntries.map((entry: any) => ({
                    character_id: entry.character_id,
                    order: newOrder,
                }));
            } else {
                savedOrder = newOrder;
            }

            // 修正：prompt_order 是与 prompts 同级的属性，不应 push 到 prompts 数组中
            // promptsToSave.push({
            //     prompt_order: savedOrder
            // });

            // 更新 presetData.prompts
            presetData.prompts = promptsToSave;
            // 更新根级 prompt_order
            presetData.prompt_order = savedOrder;

            const payload = {
                title: presetTitle,
                user_note: userNote,
                version: version,
                data: presetData, // 包含更新后的 prompts 和 prompt_order
                regex_data: regexData,
                pipi_study: pipiStudy,
            };

            const res = await fetch(`${API_BASE}/api/presets/${id}`, {
                method: "PATCH",
                headers: {
                    "Content-Type": "application/json",
                    ...(token ? { Authorization: `Bearer ${token}` } : {}),
                },
                body: JSON.stringify(payload),
            });

            if (!res.ok) throw new Error("保存失败");

            item = await res.json();
            presetTitle = item.title;
            userNote = item.user_note || "";

            // 重新解析数据
            try {
                presetData = JSON.parse(item.data);
            } catch (e) {
                /* keep current */
            }
            try {
                regexData = JSON.parse(item.regex_data);
                if (!Array.isArray(regexData)) regexData = [];
            } catch (e) {
                /* keep current */
            }

            // 重新应用 loadItem 的分组逻辑吗？不，我们保留当前状态，只是更新 meta
            // 但为了安全起见，我们重新同步一下状态快照
            snapshotState();
            snapshotPrompts();
            snapshotRegex();
            isDirty = false;
            lastSaved = Date.now();
            toast.success("保存成功");

            breadcrumbs.set([
                { label: "预设", href: "/presets" },
                { label: presetTitle },
            ]);
        } catch (e) {
            console.error(e);
            toast.error("保存失败");
        } finally {
            saving = false;
        }
    }

    // --- 导出 ---
    function exportPreset(url: string, filename: string) {
        const token = localStorage.getItem("auth_token");
        downloadFile({
            url,
            filename,
            fetchOptions: token ? { headers: { Authorization: `Bearer ${token}` } } : undefined,
        });
    }

    // --- 导入正则 ---
    function triggerImportRegex() {
        fileInput?.click();
    }

    async function handleImportRegex(event: Event) {
        const input = event.target as HTMLInputElement;
        const files = input.files;
        if (!files || files.length === 0) return;

        const readFile = (file: File): Promise<any[]> => {
            return new Promise((resolve) => {
                const reader = new FileReader();
                reader.onload = (e) => {
                    try {
                        const content = e.target?.result as string;
                        let imported = JSON.parse(content);
                        let scripts: any[] = [];

                        // 兼容不同格式：
                        // 1. 纯数组
                        // 2. ST 格式 { regex_scripts: [...] }
                        if (Array.isArray(imported)) {
                            scripts = imported;
                        } else if (imported.regex_scripts && Array.isArray(imported.regex_scripts)) {
                            scripts = imported.regex_scripts;
                        } else if (imported.scripts && Array.isArray(imported.scripts)) {
                            scripts = imported.scripts;
                        } else if (typeof imported === "object" && imported !== null) {
                            // 单个对象的情况
                            scripts = [imported];
                        }

                        // 处理导入的脚本，确保有 ID
                        const processed = scripts.map((s: any) => ({
                            ...s,
                            id: s.id || generateUUID(),
                            // 确保必要字段存在
                            scriptName: s.scriptName || "导入的正则",
                            findRegex: s.findRegex || "",
                            replaceString: s.replaceString || "",
                            placement: s.placement || [2],
                            disabled: s.disabled || false,
                        }));
                        resolve(processed);
                    } catch (err) {
                        console.error(err);
                        toast.error(`解析文件 ${file.name} 失败`);
                        resolve([]);
                    }
                };
                reader.onerror = () => {
                    toast.error(`读取文件 ${file.name} 失败`);
                    resolve([]);
                };
                reader.readAsText(file);
            });
        };

        try {
            const promises = Array.from(files).map(readFile);
            const results = await Promise.all(promises);
            const allScripts = results.flat();

            if (allScripts.length > 0) {
                regexData = [...regexData, ...allScripts];
                checkDirty();
                toast.success(`成功从 ${files.length} 个文件中导入 ${allScripts.length} 条正则`);
            } else {
                toast.warning("未能识别到有效的正则数据");
            }
        } catch (err) {
            console.error(err);
            toast.error("批量导入过程发生错误");
        } finally {
            input.value = "";
        }
    }

    function addRegex() {
        const newScript = {
            id: generateUUID(),
            scriptName: "新正则",
            findRegex: "",
            replaceString: "",
            trimStrings: [],
            placement: [2],
            disabled: false,
            markdownOnly: true,
            promptOnly: false,
            runOnEdit: true,
            substituteRegex: 0,
            minDepth: null,
            maxDepth: null,
        };
        regexData = [...regexData, newScript];
        checkDirty();
        toast.success("已添加新正则");
    }

    function deleteRegex(scriptId: string) {
        regexData = regexData.filter((s: any) => s.id !== scriptId);
        checkDirty();
        toast.success("正则已删除");
    }

    // --- 正则拖拽 ---
    let orderBeforeDrag: string[] = [];

    function handleDndConsider(e: CustomEvent<{ items: any[]; info: { trigger: string } }>) {
        if (e.detail.info.trigger === TRIGGERS.DRAG_STARTED) {
            orderBeforeDrag = regexData.map((s: any) => s.id);
            isDragging = true;
            dndItems = e.detail.items;
        } else if (isDragging) {
            dndItems = e.detail.items;
        }
    }

    function handleDndFinalize(e: CustomEvent<{ items: any[]; info: { trigger: string } }>) {
        if (!isDragging) return;
        isDragging = false;

        const newOrder = e.detail.items.map((s: any) => s.id);
        const orderChanged =
            orderBeforeDrag.length > 0 &&
            (orderBeforeDrag.length !== newOrder.length ||
                orderBeforeDrag.some((id, i) => id !== newOrder[i]));

        if (orderChanged) {
            regexData = e.detail.items.map((item) => {
                const { isDndShadowItem, ...cleanItem } = item;
                return cleanItem;
            });
            checkDirty();
        }

        dndItems = [];
        orderBeforeDrag = [];
    }

    let displayRegex = $derived(isDragging ? dndItems : regexData);

    // --- 条目修改触发脏状态 ---
    function onPromptChange() {
        checkDirty();
    }

    function onRegexFieldChange() {
        checkDirty();
    }

    // 作用范围选项
    const PLACEMENTS = [
        { value: 1, label: "用户输入" },
        { value: 2, label: "AI输出" },
        { value: 3, label: "快捷命令" },
        { value: 5, label: "世界信息" },
        { value: 6, label: "推理" },
    ];

    // 主列表中的开关：切换 prompt_order 中的 enabled 值
    function togglePromptEnabled(prompt: any, enabled: boolean) {
        if (prompt.enabled === enabled) return;
        prompt.enabled = enabled;
        onPromptChange();
    }

    // 未启用列表中的开关：将条目加入/移出 prompt_order
    function setPromptEnabled(prompt: any, inOrder: boolean) {
        if (prompt._inOrder === inOrder) return;
        
        prompt._inOrder = inOrder;
        
        if (inOrder) {
            // 从未启用移到主列表，默认 enabled=true
            prompt.enabled = true;
            if (disabledPrompts.includes(prompt)) {
                disabledPrompts = disabledPrompts.filter(p => p !== prompt);
                enabledPrompts = [...enabledPrompts, prompt];
            }
        } else {
            // 从主列表移到未启用列表
            if (enabledPrompts.includes(prompt)) {
                enabledPrompts = enabledPrompts.filter(p => p !== prompt);
                disabledPrompts = [prompt, ...disabledPrompts];
            }
        }
        onPromptChange();
    }

    // --- 排序模式 ---
    let isSortingMode = $state(false);
    let isPromptDragging = $state(false);
    let promptDndItems: any[] = $state([]);
    let flipDurationMs = $state(0);

    // 防抖的脏检查
    let dirtyCheckTimer: any = null;
    function checkDirtyDebounced() {
        if (dirtyCheckTimer) clearTimeout(dirtyCheckTimer);
        dirtyCheckTimer = setTimeout(() => {
            checkDirty();
        }, 300);
    }

    function toggleSortMode() {
        isSortingMode = !isSortingMode;
        if (isSortingMode) {
            // 优化方案三：轻量化拖拽对象
            // 仅提取必要字段，避免大对象（content可能很大）参与 dnd 计算和渲染
            promptDndItems = enabledPrompts.map(p => ({
                id: p.identifier, // dndzone core
                identifier: p.identifier,
                name: p.name
            }));
            
            // 优化方案一：根据列表长度决定是否启用动画
            // 列表过长时关闭动画以提升性能
            flipDurationMs = enabledPrompts.length > 50 ? 0 : 200;
        } else {
            promptDndItems = [];
        }
    }

    function handlePromptDndConsider(e: CustomEvent<DndEvent<any>>) {
        promptDndItems = e.detail.items;
        isPromptDragging = true;
    }

    function handlePromptDndFinalize(e: CustomEvent<DndEvent<any>>) {
        promptDndItems = e.detail.items;
        isPromptDragging = false;
        
        // 优化方案三：还原完整对象
        // 根据排序后的 id (identifier) 列表，从原始 enabledPrompts 中找回完整对象
        const promptMap = new Map(enabledPrompts.map(p => [p.identifier, p]));
        const newEnabledPrompts: any[] = [];
        
        for (const item of e.detail.items) {
            const original = promptMap.get(item.identifier);
            if (original) {
                newEnabledPrompts.push(original);
            }
        }
        
        enabledPrompts = newEnabledPrompts;
        
        // 优化方案二：防抖检查
        // 拖拽结束肯定变脏了，可以直接设 true，或者防抖检查内容
        // 这里为了精确性（比如拖了一圈又回去了），还是做一下检查，但是防抖
        checkDirtyDebounced(); 
    }
</script>

<div class="container mx-auto py-6 space-y-6 flex flex-col max-w-7xl">
    <UnsavedGuard controller={unsaved} />

    <!-- 顶部 -->
    <div class="flex flex-col gap-3 shrink-0">
        <!-- 标题行 -->
        <div class="flex items-center gap-3 min-w-0">
            <Button variant="ghost" size="icon" href="/presets" class="shrink-0">
                <ArrowLeft class="h-5 w-5" />
            </Button>
            <h1 class="text-xl md:text-2xl font-bold tracking-tight truncate">
                {presetTitle || item?.title || "加载中..."}
            </h1>
            {#if isDirty}
                <span class="text-sm text-yellow-500 font-medium whitespace-nowrap">（未保存）</span>
            {/if}
        </div>
        <!-- 操作按钮行 -->
        <div class="flex items-center gap-2 flex-wrap pl-0 md:pl-11 ml-auto">
            <!-- 缝合 -->
            <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                    <Button variant="outline" size="sm" class="gap-1.5">
                        <Combine class="h-4 w-4" /> 缝合
                    </Button>
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="start">
                    <DropdownMenu.Item
                        onclick={() => {
                            stitchMode = "to_other";
                            stitchModalOpen = true;
                        }}
                    >
                        <ArrowRight class="h-4 w-4 mr-2" />
                        将条目缝合到其他预设
                    </DropdownMenu.Item>
                    <DropdownMenu.Item
                        onclick={() => {
                            stitchMode = "from_other";
                            stitchModalOpen = true;
                        }}
                    >
                        <ArrowRight class="h-4 w-4 mr-2 rotate-180" />
                        缝合其他预设的条目到本预设
                    </DropdownMenu.Item>
                </DropdownMenu.Content>
            </DropdownMenu.Root>

            <!-- 导出 -->
            <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                    <Button variant="outline" size="sm" class="gap-1.5">
                        <Download class="h-4 w-4" /> 导出
                    </Button>
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="start">
                    <DropdownMenu.Item
                        onclick={() =>
                            exportPreset(
                                `${API_BASE}/api/presets/${id}/export`,
                                `${presetTitle}.json`,
                            )}
                    >
                        <Download class="h-4 w-4 mr-2" />
                        导出预设（包含正则）
                    </DropdownMenu.Item>
                    <DropdownMenu.Item
                        onclick={() =>
                            exportPreset(
                                `${API_BASE}/api/presets/${id}/export-regex`,
                                `${presetTitle}_regex.json`,
                            )}
                    >
                        <FileJson class="h-4 w-4 mr-2" />
                        仅导出正则包
                    </DropdownMenu.Item>
                </DropdownMenu.Content>
            </DropdownMenu.Root>

            <!-- 保存 -->
            <Button disabled={saving || !isDirty} onclick={save} size="sm" class="gap-1.5">
                {#if saving}
                    <Loader2 class="h-4 w-4 animate-spin" /> 保存中...
                {:else}
                    <Save class="h-4 w-4" /> 保存
                {/if}
            </Button>
        </div>
    </div>

    <!-- 内容 -->
    {#if loading}
        <div class="flex-1 flex items-center justify-center text-muted-foreground">
            <Loader2 class="h-8 w-8 animate-spin" />
        </div>
    {:else}
        <Tabs.Root value={activeTab} onValueChange={(v) => (activeTab = v)}>
            <Tabs.List class="grid w-full grid-cols-3 max-w-md">
                <Tabs.Trigger value="prompts">预设条目</Tabs.Trigger>
                <Tabs.Trigger value="regex">配套正则</Tabs.Trigger>
                <Tabs.Trigger value="pipi_study">小皮书童</Tabs.Trigger>
            </Tabs.List>

            <!-- ===== 预设条目 Tab ===== -->
            <Tabs.Content value="prompts" class="space-y-6 mt-4">
                <!-- 标题和备注 -->
                <!-- 标题、版本和备注 -->
                <div class="grid gap-4 sm:grid-cols-4">
                    <div class="space-y-2 sm:col-span-3">
                        <Label>预设名称</Label>
                        <DirtyInput
                            value={presetTitle}
                            isDirty={presetTitle !== initialTitle}
                            oninput={(e) => {
                                presetTitle = e.currentTarget.value;
                                onPromptChange();
                            }}
                            placeholder="预设名称"
                        />
                    </div>
                    <div class="space-y-2 sm:col-span-1">
                        <Label>版本号</Label>
                        <DirtyInput
                            value={version}
                            isDirty={version !== initialVersion}
                            oninput={(e) => {
                                // 只允许数字和小数点
                                const val = e.currentTarget.value.replace(/[^0-9.]/g, '');
                                version = val;
                                // e.currentTarget.value = val; // Svelte 5 bind:value handles this usually, but let's be safe or just rely on state
                                onPromptChange();
                            }}
                            placeholder="1.0.0"
                        />
                    </div>
                    <div class="space-y-2 sm:col-span-4">
                        <Label>简要备注</Label>
                        <DirtyInput
                            value={userNote}
                            isDirty={userNote !== initialNote}
                            oninput={(e) => {
                                userNote = e.currentTarget.value;
                                onPromptChange();
                            }}
                            placeholder="对该预设的备注（可选）"
                        />
                    </div>
                </div>

                <!-- Prompts 条目列表 -->
                <div class="space-y-6">
                     <!-- 启用的条目 -->
                     <div>
                        <h3 class="text-sm font-medium text-muted-foreground mb-3 flex items-center justify-between">
                            <span>启用 ({enabledPrompts.length})</span>
                            <div class="flex items-center gap-2">
                                <Button
                                    variant={isSortingMode ? "secondary" : "ghost"}
                                    size="sm"
                                    class="h-7 text-xs gap-1"
                                    onclick={toggleSortMode}
                                >
                                    {#if isSortingMode}
                                        <X class="h-3.5 w-3.5" /> 完成排序
                                    {:else}
                                        <ArrowUpDown class="h-3.5 w-3.5" /> 排序
                                    {/if}
                                </Button>
                            </div>
                        </h3>
                        
                        {#if isSortingMode}
                            <!-- 排序模式视图 -->
                            <div
                                class="space-y-2"
                                use:dndzone={{ items: promptDndItems, flipDurationMs, dropTargetStyle: { outline: "none" } }}
                                onconsider={handlePromptDndConsider}
                                onfinalize={handlePromptDndFinalize}
                            >
                                {#each promptDndItems as prompt (prompt.id)}
                                    <div
                                        class="flex items-center gap-3 p-3 rounded-lg border bg-card shadow-sm select-none"
                                        animate:flip={{ duration: flipDurationMs }}
                                    >
                                        <GripVertical class="h-4 w-4 text-muted-foreground/50 shrink-0 cursor-move" />
                                        <span class="text-xs font-mono text-muted-foreground/70 w-6 text-right">
                                            {enabledPrompts.findIndex(p => p.identifier === prompt.identifier) + 1}
                                        </span>
                                        <span class="font-medium text-sm truncate flex-1">
                                            {prompt.name || "未命名条目"}
                                        </span>
                                        <!-- 排序模式下用 Switch 也可以，但最好只是展示，避免复杂交互混乱，这里做成只读或者禁用 -->
                                        <Switch checked={true} disabled class="scale-75 opacity-50" />
                                    </div>
                                {/each}
                            </div>
                        {:else if enabledPrompts.length > 0}
                            <div class="space-y-3">
                                {#each enabledPrompts as prompt, i (prompt.identifier || i)}
                                    <!-- 渲染单个 Prompt (提取为复用组件或保留在此) -->
                                    <div
                                        class={cn(
                                            "rounded-xl border bg-card/50 shadow-sm transition-all duration-300 group relative",
                                            openPrompts[prompt.identifier] 
                                                ? "border-primary ring-1 ring-primary/100 shadow-md bg-card"
                                                : "border-border/40 hover:!bg-accent/40 hover:!border-border/40",
                                        )}
                                    >
                                        <!-- 折叠头部 -->
                                        <div
                                            class={cn(
                                                "flex items-center gap-3 p-3 cursor-pointer transition-colors",
                                                openPrompts[prompt.identifier] ? "bg-primary/5 rounded-t-xl" : "bg-transparent group-hover:!bg-accent/40 rounded-xl",
                                            )}
                                            role="button"
                                            tabindex="0"
                                            onclick={() => (openPrompts[prompt.identifier] = !openPrompts[prompt.identifier])}
                                            onkeydown={(e) => {
                                                if (e.key === "Enter" || e.key === " ") {
                                                    e.preventDefault();
                                                    openPrompts[prompt.identifier] = !openPrompts[prompt.identifier];
                                                }
                                            }}
                                        >
                                            <!-- 启用开关 -->
                                            <div
                                                class="flex items-center gap-2 shrink-0"
                                                role="none"
                                                onclick={(e) => e.stopPropagation()}
                                                onkeydown={(e) => e.stopPropagation()}
                                            >
                                                <Switch
                                                    checked={prompt.enabled !== false}
                                                    onCheckedChange={(v) => togglePromptEnabled(prompt, v)}
                                                    class="data-[state=checked]:bg-primary data-[state=unchecked]:bg-input scale-75 origin-left"
                                                />
                                            </div>

                                            <div class={cn("transition-transform duration-200 shrink-0", openPrompts[prompt.identifier] && "rotate-180")}>
                                                <ChevronDown class="h-4 w-4 text-muted-foreground" />
                                            </div>

                                            <span class="font-medium truncate flex-1">
                                                {prompt.name || `条目`}
                                            </span>

                                            {#if prompt.role}
                                                <Badge variant="outline" class="text-[10px] shrink-0 font-normal opacity-80">{prompt.role}</Badge>
                                            {/if}

                                            <!-- 删除按钮 -->
                                            <div
                                                role="none"
                                                onclick={(e) => e.stopPropagation()}
                                                onkeydown={(e) => e.stopPropagation()}
                                            >
                                                <Button
                                                    variant="ghost"
                                                    size="icon"
                                                    class="h-7 w-7 text-muted-foreground/50 hover:text-destructive shrink-0"
                                                    onclick={() => deletePrompt(prompt)}
                                                    title="删除此条目"
                                                >
                                                    <Trash2 class="h-3.5 w-3.5" />
                                                </Button>
                                            </div>
                                        </div>

                                        <!-- 内容 -->
                                        {#if openPrompts[prompt.identifier]}
                                            <div class="p-4 border-t space-y-4">
                                                <div class="space-y-2">
                                                    <div class="flex items-center justify-between">
                                                        <Label>条目名称</Label>
                                                        <div class="flex items-center gap-2">
                                                            <span class="text-xs text-muted-foreground">启用条目</span>
                                                            <Switch
                                                                checked={prompt._inOrder !== false}
                                                                onCheckedChange={(v) => setPromptEnabled(prompt, v)}
                                                                class="scale-75 origin-right"
                                                            />
                                                        </div>
                                                    </div>
                                                    <DirtyInput
                                                        value={prompt.name}
                                                        isDirty={isFieldDirty(prompt.identifier, "name", prompt.name)}
                                                        oninput={(e) => {
                                                            prompt.name = e.currentTarget.value;
                                                            onPromptChange();
                                                        }}
                                                        placeholder="条目名称"
                                                    />
                                                </div>
                                                <div class="space-y-2">
                                                    <Label>内容</Label>
                                                    <DirtyTextarea
                                                        value={prompt.content}
                                                        isDirty={isFieldDirty(prompt.identifier, "content", prompt.content)}
                                                        oninput={(e) => {
                                                            prompt.content = e.currentTarget.value;
                                                            onPromptChange();
                                                        }}
                                                        class="font-mono text-xs min-h-[120px]"
                                                        placeholder="条目内容..."
                                                        rows={6}
                                                    />
                                                </div>
                                            </div>
                                        {/if}
                                    </div>
                                {/each}
                            </div>
                        {:else}
                             <div class="text-center py-8 text-muted-foreground text-sm border border-dashed rounded-lg bg-muted/30">
                                <p>无启用条目</p>
                            </div>
                        {/if}
                     </div>

                    <!-- 未启用的条目 (details 折叠) -->
                    <details class="group/details open:bg-muted/10 rounded-xl border border-transparent open:border-border/40 transition-all">
                        <summary class="flex items-center gap-2 p-3 font-medium text-sm text-muted-foreground cursor-pointer select-none hover:text-foreground">
                            <ChevronDown class="h-4 w-4 transition-transform group-open/details:rotate-180" />
                            <span>未启用 ({disabledPrompts.length})</span>
                        </summary>
                        
                        <div class="p-4 pt-0 space-y-3">
                             {#if disabledPrompts.length > 0}
                                {#each disabledPrompts as prompt, i (prompt.identifier || i)}
                                    <!-- 禁用的渲染 (类似启用，但样式不同，开关行为相反) -->
                                    <div
                                        class={cn(
                                            "rounded-xl border bg-muted/40 transition-all duration-300 opacity-75 hover:opacity-100",
                                            openPrompts[prompt.identifier] ? "ring-1 ring-primary/20 bg-muted/60" : ""
                                        )}
                                    >
                                        <!-- 简单头部 -->
                                        <div class="flex items-center gap-3 p-3">
                                            <div class="flex items-center gap-2 shrink-0">

                                                <Switch
                                                    checked={prompt._inOrder !== false}
                                                    onCheckedChange={(v) => setPromptEnabled(prompt, v)}
                                                    class="scale-75 origin-left"
                                                />
                                            </div>
                                            
                                            <div 
                                                class="flex-1 flex items-center gap-2 cursor-pointer"
                                                role="button"
                                                tabindex="0"
                                                onclick={() => (openPrompts[prompt.identifier] = !openPrompts[prompt.identifier])}
                                                onkeydown={(e) => {
                                                    if (e.key === "Enter" || e.key === " ") {
                                                        e.preventDefault();
                                                        openPrompts[prompt.identifier] = !openPrompts[prompt.identifier];
                                                    }
                                                }}
                                            >
                                                <ChevronDown class={cn("h-4 w-4 text-muted-foreground transition-transform", openPrompts[prompt.identifier] && "rotate-180")} />
                                                <span class="font-medium truncate text-muted-foreground line-through decoration-muted-foreground/50">
                                                    {prompt.name || `条目`}
                                                </span>
                                            </div>

                                            <!-- 删除按钮 -->
                                            <Button
                                                variant="ghost"
                                                size="icon"
                                                class="h-7 w-7 text-muted-foreground/50 hover:text-destructive shrink-0"
                                                onclick={() => deletePrompt(prompt)}
                                                title="删除此条目"
                                            >
                                                <Trash2 class="h-3.5 w-3.5" />
                                            </Button>
                                        </div>

                                        <!-- 内容区域 -->
                                        {#if openPrompts[prompt.identifier]}
                                            <div class="p-4 border-t space-y-4 bg-background/50">
                                                <!-- 同上编辑字段 -->
                                                <div class="space-y-2">
                                                    <div class="flex items-center justify-between">
                                                        <Label>条目名称</Label>
                                                        <div class="flex items-center gap-2">
                                                            <span class="text-xs text-muted-foreground">启用条目</span>
                                                            <Switch
                                                                checked={prompt._inOrder !== false}
                                                                onCheckedChange={(v) => setPromptEnabled(prompt, v)}
                                                                class="scale-75 origin-right"
                                                            />
                                                        </div>
                                                    </div>
                                                    <DirtyInput
                                                        value={prompt.name}
                                                        isDirty={isFieldDirty(prompt.identifier, "name", prompt.name)}
                                                        oninput={(e) => {
                                                            prompt.name = e.currentTarget.value;
                                                            onPromptChange();
                                                        }}
                                                    />
                                                </div>
                                                <div class="space-y-2">
                                                    <Label>内容</Label>
                                                    <DirtyTextarea
                                                        value={prompt.content}
                                                        isDirty={isFieldDirty(prompt.identifier, "content", prompt.content)}
                                                        oninput={(e) => {
                                                            prompt.content = e.currentTarget.value;
                                                            onPromptChange();
                                                        }}
                                                        rows={4}
                                                    />
                                                </div>
                                            </div>
                                        {/if}
                                    </div>
                                {/each}
                             {:else}
                                <p class="text-xs text-muted-foreground pl-2">没有未启用的条目</p>
                             {/if}
                        </div>
                    </details>
                </div>

            </Tabs.Content>

            <!-- ===== 配套正则 Tab ===== -->
            <Tabs.Content value="regex" class="space-y-4 mt-4">
                <!-- 工具栏 -->
                <div class="flex items-center justify-between gap-4 px-1">
                    <p class="text-sm text-muted-foreground">
                        共 {regexData.length} 个正则
                    </p>
                    <div class="flex items-center gap-2">
                        <Button
                            size="sm"
                            variant="outline"
                            class="gap-2 border-dashed"
                            onclick={triggerImportRegex}
                        >
                            <Upload class="h-4 w-4" />
                            <span class="hidden sm:inline">导入正则</span>
                        </Button>
                        <Button
                            size="sm"
                            class="gap-2 border-primary bg-background text-foreground hover:bg-primary/10"
                            onclick={addRegex}
                            variant="outline"
                        >
                            <Plus class="h-4 w-4" />
                            <span class="hidden sm:inline">添加正则</span>
                        </Button>
                    </div>
                </div>

                <!-- 隐藏的文件输入 -->
                <input
                    bind:this={fileInput}
                    type="file"
                    accept=".json"
                    multiple
                    class="hidden"
                    onchange={handleImportRegex}
                />

                <!-- 正则列表 -->
                <div class="space-y-3 pb-8">
                    {#if regexData.length === 0}
                        <div class="flex flex-col items-center justify-center py-12 text-muted-foreground text-sm border border-dashed rounded-lg bg-muted/30">
                            <RegexIcon class="h-8 w-8 mb-2 opacity-50" />
                            <p>暂无配套正则</p>
                        </div>
                    {:else}
                        <div
                            use:dndzone={{
                                items: displayRegex,
                                flipDurationMs: FLIP_DURATION_MS,
                                delayTouchStart: TOUCH_DELAY_MS,
                                dragDisabled: Object.values(openScripts).some((v) => v),
                                dropTargetStyle: {},
                                type: "preset-regex",
                            }}
                            onconsider={handleDndConsider}
                            onfinalize={handleDndFinalize}
                            class="space-y-3"
                        >
                            {#each displayRegex as script (script.id)}
                                {@const realIndex = script.isDndShadowItem ? -1 : regexData.findIndex((s) => s.id === script.id)}
                                <div
                                    animate:flip={{ duration: FLIP_DURATION_MS }}
                                    class={cn(
                                        "transition-all duration-200 relative",
                                        script.isDndShadowItem && "h-16 rounded-xl border-2 border-dashed border-primary/50 bg-primary/5",
                                        !script.isDndShadowItem && openScripts[script.id] ? "z-20" : "z-0 hover:!z-50",
                                    )}
                                >
                                    {#if !script.isDndShadowItem && realIndex !== -1}
                                        <!-- 正则项 -->
                                        <div
                                            class={cn(
                                                "rounded-xl border bg-card/50 shadow-sm transition-all duration-300 group relative",
                                                openScripts[script.id]
                                                    ? "border-primary ring-1 ring-primary/100 shadow-md bg-card z-20"
                                                    : "border-border/40 hover:!bg-accent/40 hover:!border-border/40 hover:!z-50",
                                            )}
                                        >
                                            <!-- 头部 -->
                                            <div
                                                class={cn(
                                                    "sticky top-0 z-10 flex items-center gap-3 p-3 transition-colors cursor-pointer",
                                                    openScripts[script.id] ? "bg-primary/5 rounded-t-xl" : "bg-transparent group-hover:!bg-accent/40 rounded-xl",
                                                )}
                                                role="button"
                                                tabindex="0"
                                                onclick={() => (openScripts[script.id] = !openScripts[script.id])}
                                                onkeydown={(e) => {
                                                    if (e.key === "Enter" || e.key === " ") {
                                                        e.preventDefault();
                                                        openScripts[script.id] = !openScripts[script.id];
                                                    }
                                                }}
                                            >
                                                <!-- 拖拽手柄 -->
                                                <div class="cursor-grab active:cursor-grabbing text-muted-foreground hover:text-foreground shrink-0">
                                                    <GripVertical class="h-4 w-4" />
                                                </div>

                                                <!-- 启用开关 -->
                                                <div
                                                    class="flex items-center gap-2 shrink-0"
                                                    role="none"
                                                    onclick={(e) => e.stopPropagation()}
                                                    onkeydown={(e) => e.stopPropagation()}
                                                >
                                                    <Switch
                                                        checked={!regexData[realIndex].disabled}
                                                        onCheckedChange={(v) => {
                                                            regexData[realIndex].disabled = !v;
                                                            onRegexFieldChange();
                                                        }}
                                                        class="data-[state=checked]:bg-primary data-[state=unchecked]:bg-input scale-75 origin-left"
                                                    />
                                                </div>

                                                <!-- 展开图标 -->
                                                <div class={cn("transition-transform duration-200 shrink-0", openScripts[script.id] && "rotate-180")}>
                                                    <ChevronDown class="h-4 w-4 text-muted-foreground" />
                                                </div>

                                                <!-- 名称 -->
                                                <span
                                                    class={cn(
                                                        "font-medium truncate flex-1",
                                                        regexData[realIndex].disabled && "text-muted-foreground line-through decoration-muted-foreground/50",
                                                    )}
                                                >
                                                    {regexData[realIndex].scriptName || "未命名正则"}
                                                </span>

                                                <!-- 删除 -->
                                                <div
                                                    role="none"
                                                    onclick={(e) => e.stopPropagation()}
                                                    onkeydown={(e) => e.stopPropagation()}
                                                >
                                                    <Button
                                                        variant="ghost"
                                                        size="icon"
                                                        class="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                                                        onclick={() => deleteRegex(script.id)}
                                                    >
                                                        <Trash2 class="h-4 w-4" />
                                                    </Button>
                                                </div>
                                            </div>

                                            <!-- 展开内容 -->
                                            {#if openScripts[script.id]}
                                                <div class="p-4 border-t space-y-4">
                                                    <div class="grid gap-4 sm:grid-cols-2">
                                                        <!-- 正则名称 -->
                                                        <div class="space-y-2 sm:col-span-2">
                                                            <Label>正则名称</Label>
                                                            <DirtyInput
                                                                value={regexData[realIndex].scriptName}
                                                                isDirty={isRegexFieldDirty(script.id, "scriptName", regexData[realIndex].scriptName)}
                                                                oninput={(e) => {
                                                                    regexData[realIndex].scriptName = e.currentTarget.value;
                                                                    onRegexFieldChange();
                                                                }}
                                                                placeholder="例如: 移除多余空行"
                                                            />
                                                        </div>

                                                        <!-- 正则表达式 -->
                                                        <div class="space-y-2 sm:col-span-2">
                                                            <Label>正则表达式</Label>
                                                            <DirtyTextarea
                                                                value={regexData[realIndex].findRegex}
                                                                isDirty={isRegexFieldDirty(script.id, "findRegex", regexData[realIndex].findRegex)}
                                                                oninput={(e) => {
                                                                    regexData[realIndex].findRegex = e.currentTarget.value;
                                                                    onRegexFieldChange();
                                                                }}
                                                                class="font-mono text-xs min-h-[60px]"
                                                                rows={2}
                                                                placeholder="在这里写正则表达式..."
                                                            />
                                                        </div>

                                                        <!-- 替换为 -->
                                                        <div class="space-y-2 sm:col-span-2">
                                                            <Label>替换为</Label>
                                                            <DirtyTextarea
                                                                value={regexData[realIndex].replaceString}
                                                                isDirty={isRegexFieldDirty(script.id, "replaceString", regexData[realIndex].replaceString)}
                                                                oninput={(e) => {
                                                                    regexData[realIndex].replaceString = e.currentTarget.value;
                                                                    onRegexFieldChange();
                                                                }}
                                                                class="font-mono text-xs min-h-[60px]"
                                                                rows={3}
                                                                placeholder="在这里写替换内容..."
                                                            />
                                                        </div>

                                                        <!-- 作用范围 -->
                                                        <div class="space-y-2 sm:col-span-2">
                                                            <Label>作用范围</Label>
                                                            <div class="border rounded-md p-3">
                                                                <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                                                                    {#each PLACEMENTS as p}
                                                                        <div class="flex items-center space-x-2">
                                                                            <input
                                                                                type="checkbox"
                                                                                id={`placement-${script.id}-${p.value}`}
                                                                                checked={regexData[realIndex].placement?.includes(p.value)}
                                                                                onchange={(e) => {
                                                                                    const checked = (e.target as HTMLInputElement).checked;
                                                                                    if (!regexData[realIndex].placement) regexData[realIndex].placement = [];
                                                                                    if (checked) {
                                                                                        if (!regexData[realIndex].placement.includes(p.value)) {
                                                                                            regexData[realIndex].placement = [...regexData[realIndex].placement, p.value];
                                                                                        }
                                                                                    } else {
                                                                                        regexData[realIndex].placement = regexData[realIndex].placement.filter((v: number) => v !== p.value);
                                                                                    }
                                                                                    onRegexFieldChange();
                                                                                }}
                                                                                class="h-4 w-4 rounded border-gray-300 text-primary accent-primary"
                                                                            />
                                                                            <label for={`placement-${script.id}-${p.value}`} class="text-xs font-normal cursor-pointer text-muted-foreground">
                                                                                {p.label}
                                                                            </label>
                                                                        </div>
                                                                    {/each}
                                                                </div>
                                                            </div>
                                                        </div>

                                                        <!-- 深度 -->
                                                        <div class="space-y-2">
                                                            <Label>最小深度</Label>
                                                            <DirtyInput
                                                                type="number"
                                                                value={regexData[realIndex].minDepth}
                                                                isDirty={isRegexFieldDirty(script.id, "minDepth", regexData[realIndex].minDepth)}
                                                                oninput={(e) => {
                                                                    const val = e.currentTarget.value;
                                                                    regexData[realIndex].minDepth = val === "" ? null : Number(val);
                                                                    onRegexFieldChange();
                                                                }}
                                                                placeholder="无限制"
                                                            />
                                                        </div>
                                                        <div class="space-y-2">
                                                            <Label>最大深度</Label>
                                                            <DirtyInput
                                                                type="number"
                                                                value={regexData[realIndex].maxDepth}
                                                                isDirty={isRegexFieldDirty(script.id, "maxDepth", regexData[realIndex].maxDepth)}
                                                                oninput={(e) => {
                                                                    const val = e.currentTarget.value;
                                                                    regexData[realIndex].maxDepth = val === "" ? null : Number(val);
                                                                    onRegexFieldChange();
                                                                }}
                                                                placeholder="无限制"
                                                            />
                                                        </div>

                                                        <!-- 标记选项 -->
                                                        <div class="sm:col-span-2 space-y-3 pt-2">
                                                            <div class="flex flex-wrap gap-4">
                                                                <div class="flex items-center space-x-2">
                                                                    <input
                                                                        type="checkbox"
                                                                        id={`md-${script.id}`}
                                                                        checked={!!regexData[realIndex].markdownOnly}
                                                                        onchange={(e) => {
                                                                            regexData[realIndex].markdownOnly = (e.target as HTMLInputElement).checked;
                                                                            onRegexFieldChange();
                                                                        }}
                                                                        class="h-4 w-4 rounded border-gray-300 text-primary accent-primary"
                                                                    />
                                                                    <label for={`md-${script.id}`} class="text-sm font-normal cursor-pointer">仅格式显示</label>
                                                                </div>
                                                                <div class="flex items-center space-x-2">
                                                                    <input
                                                                        type="checkbox"
                                                                        id={`pmt-${script.id}`}
                                                                        checked={!!regexData[realIndex].promptOnly}
                                                                        onchange={(e) => {
                                                                            regexData[realIndex].promptOnly = (e.target as HTMLInputElement).checked;
                                                                            onRegexFieldChange();
                                                                        }}
                                                                        class="h-4 w-4 rounded border-gray-300 text-primary accent-primary"
                                                                    />
                                                                    <label for={`pmt-${script.id}`} class="text-sm font-normal cursor-pointer">仅格式提示词</label>
                                                                </div>
                                                                <div class="flex items-center space-x-2">
                                                                    <input
                                                                        type="checkbox"
                                                                        id={`run-${script.id}`}
                                                                        checked={regexData[realIndex].runOnEdit !== false}
                                                                        onchange={(e) => {
                                                                            regexData[realIndex].runOnEdit = (e.target as HTMLInputElement).checked;
                                                                            onRegexFieldChange();
                                                                        }}
                                                                        class="h-4 w-4 rounded border-gray-300 text-primary accent-primary"
                                                                    />
                                                                    <label for={`run-${script.id}`} class="text-sm font-normal cursor-pointer">在编辑时运行</label>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    </div>
                                                </div>
                                            {/if}
                                        </div>
                                    {/if}
                                </div>
                            {/each}
                        </div>
                    {/if}
                </div>
            </Tabs.Content>

            <!-- ===== 小皮书童 Tab ===== -->
            <Tabs.Content value="pipi_study" class="space-y-6 mt-4">
                <PipiStudyTab
                    presetId={$page.params.id || ""}
                    {presetData}
                    {pipiStudy}
                />
            </Tabs.Content>
        </Tabs.Root>
    {/if}

    <!-- 缝合弹窗 -->
    <StitchModal
        bind:open={stitchModalOpen}
        bind:mode={stitchMode}
        currentPresetId={id}
        currentPresetTitle={presetTitle}
        currentItems={[...enabledPrompts, ...disabledPrompts]} 
        onSave={handleStitchSave}
    />
</div>

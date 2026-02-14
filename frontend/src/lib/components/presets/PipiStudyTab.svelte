<script lang="ts">
    import { onMount } from "svelte";
    import panzoom from "panzoom";
    import { Button } from "$lib/components/ui/button";
    import { Badge } from "$lib/components/ui/badge";
    import { Separator } from "$lib/components/ui/separator";
    import { Loader2, Sparkles, RefreshCw, BookOpen, Layers, Lightbulb, Zap, Scissors, X, FileText } from "lucide-svelte";
    import { toast } from "svelte-sonner";
    import { AiService } from "$lib/ai/service";
    import { api } from "$lib/api";
    import mermaid from "mermaid";
    import { marked } from "marked";

    // Props
    let {
        presetId,
        presetData,
        pipiStudy = "",
    }: {
        presetId: string;
        presetData: any;
        pipiStudy: string;
    } = $props();

    // 状态
    let loading = $state(false);
    let studyData: any = $state(null);
    let mermaidRendered = $state(false);
    let zoomController: any = null; // panzoom 实例

    // 条目内容浮窗状态
    let popoverVisible = $state(false);
    let popoverIdentifier = $state("");
    let popoverContent = $state("");
    let popoverName = $state("");

    // 根据 identifier 查找条目内容
    function lookupPromptContent(identifier: string) {
        if (!presetData?.prompts) return null;
        const entry = presetData.prompts.find(
            (p: any) => p.identifier === identifier || p.name === identifier
        );
        return entry || null;
    }

    // 打开条目浮窗
    function openIdentifierPopover(identifier: string) {
        const entry = lookupPromptContent(identifier);
        if (!entry) {
            toast.info(`未找到标识符为 "${identifier}" 的条目`);
            return;
        }
        popoverIdentifier = identifier;
        popoverName = entry.name || identifier;
        popoverContent = entry.content || "（该条目内容为空）";
        popoverVisible = true;
    }

    // 关闭浮窗
    function closePopover() {
        popoverVisible = false;
    }

    // 配置 marked
    marked.setOptions({
        gfm: true,
        breaks: true,
    });

    // 获取当前的主题颜色
    function getThemeColors() {
        const isDark = document.documentElement.classList.contains('dark');
        const fg = isDark ? "#e2e8f0" : "#020817";
        const muted = isDark ? "#94a3b8" : "#64748b"; 
        const bkg = isDark ? "#020817" : "#ffffff"; 
        return { fg, muted, bkg };
    }

    // 初始化 mermaid
    onMount(() => {
        const { fg, muted, bkg } = getThemeColors();
        
        mermaid.initialize({
            startOnLoad: false,
            theme: "base", 
            securityLevel: 'loose',
            themeVariables: {
                darkMode: document.documentElement.classList.contains('dark'),
                background: "transparent",
                mainBkg: "transparent",
                
                primaryColor: "transparent", 
                primaryTextColor: fg,
                primaryBorderColor: fg,
                lineColor: fg,
                
                secondaryColor: "transparent",
                tertiaryColor: "transparent",
                
                nodeBorder: fg,
                clusterBkg: "transparent",
                clusterBorder: fg,
                
                titleColor: fg,
                edgeLabelBackground: bkg, 
                
                fontFamily: "ui-sans-serif, system-ui, sans-serif",
                fontSize: "14px", // 恢复适中两大小，依靠缩放控制
            },
            flowchart: {
                curve: 'monotoneX',
                padding: 15,
                htmlLabels: true,
                nodeSpacing: 40,
                rankSpacing: 40,
            }
        });

        // 尝试加载已有数据
        if (pipiStudy) {
            try {
                studyData = typeof pipiStudy === "string" ? JSON.parse(pipiStudy) : pipiStudy;
            } catch {
                studyData = null;
            }
        }
        
        return () => {
            if (zoomController) {
                zoomController.dispose();
            }
        };
    });

    // Mermaid 渲染
    $effect(() => {
        if (studyData?.structure_blueprint?.mermaid_code && !mermaidRendered) {
             // 渲染前销毁旧的 panzoom
            if (zoomController) {
                zoomController.dispose();
                zoomController = null;
            }
            setTimeout(renderMermaid, 100);
        }
    });

    async function renderMermaid() {
        const container = document.getElementById("mermaid-wrapper");
        const el = document.getElementById("mermaid-container");
        if (!container || !el || !studyData?.structure_blueprint?.mermaid_code) return;

        try {
            const { fg } = getThemeColors();
            
            // 每次渲染前重置内容
            el.innerHTML = '';
            el.style.transform = ''; // 重置变换

            const code = studyData.structure_blueprint.mermaid_code;
            const { svg } = await mermaid.render("mermaid-graph-" + Date.now(), code);
            el.innerHTML = svg;
            mermaidRendered = true;

            // 初始化 panzoom
            if (el.firstElementChild) {
                const svgElement = el.firstElementChild as SVGElement;
                svgElement.style.width = "100%";
                svgElement.style.height = "auto";
                
                // 初始化 panzoom
                zoomController = panzoom(el, {
                    maxZoom: 5,
                    minZoom: 0.1,
                    bounds: false,
                    boundsPadding: 0.1,
                    zoomDoubleClickSpeed: 1, // 禁用双击缩放
                    initialZoom: 1,
                    filterKey: function() { return true; } // 允许所有按键
                });
                
                // 自动缩放以适应容器宽度的 90%
                // 稍微延迟以确保 SVG 已渲染且有尺寸
                setTimeout(() => {
                    const containerRect = container.getBoundingClientRect();
                    const contentRect = svgElement.getBoundingClientRect();
                    
                    if (contentRect.width > 0 && containerRect.width > 0) {
                        const scale = (containerRect.width * 0.9) / contentRect.width;
                        // 限制初始缩放不要太大或太小
                        const finalScale = Math.min(Math.max(scale, 0.2), 1.5);
                        
                        // 计算居中偏移
                        const centerX = (containerRect.width - contentRect.width * finalScale) / 2;
                        const centerY = 20; // 顶部留一点空隙

                        zoomController.moveTo(0, 0); // 重置位置
                        zoomController.zoomAbs(0, 0, finalScale); // 缩放
                        zoomController.moveTo(centerX, centerY); // 移动
                    }
                }, 50);
            }

        } catch (e) {
            console.error("Mermaid 渲染失败:", e);
            el.innerHTML = `<pre class="text-xs text-destructive whitespace-pre-wrap p-4 border border-destructive/20 rounded-md bg-destructive/10">${studyData.structure_blueprint.mermaid_code}</pre>`;
            mermaidRendered = true;
        }
    }
    
    // 缩放控制函数
    function handleZoomIn() {
        if (zoomController) {
            const wrapper = document.getElementById("mermaid-wrapper");
            if (wrapper) {
                 const rect = wrapper.getBoundingClientRect();
                 const cx = rect.width / 2;
                 const cy = rect.height / 2;
                 zoomController.smoothZoom(cx, cy, 1.2);
            }
        }
    }

    function handleZoomOut() {
        if (zoomController) {
             const wrapper = document.getElementById("mermaid-wrapper");
            if (wrapper) {
                 const rect = wrapper.getBoundingClientRect();
                 const cx = rect.width / 2;
                 const cy = rect.height / 2;
                 zoomController.smoothZoom(cx, cy, 0.8);
            }
        }
    }
    
    function handleResetZoom() {
        if (zoomController) {
           renderMermaid(); // 重新渲染并重置是最简单的复位方法
        }
    }

    // 生成分析
    async function generateStudy() {
        if (!presetData?.prompts || presetData.prompts.length === 0) {
            toast.error("当前预设没有条目");
            return;
        }

        loading = true;
        mermaidRendered = false;
        try {
            const result = await AiService.analyzePreset(presetData);
            studyData = result;

            const saved = JSON.stringify(result);
            await api.patch(`/presets/${presetId}`, { pipi_study: saved });
            toast.success("分析完成并已保存");
        } catch (e: any) {
            console.error("预设分析失败:", e);
            toast.error(e.message || "分析失败");
        } finally {
            loading = false;
        }
    }

    function md(text: string) {
        if (!text) return "";
        try {
            // 使用 div 包裹 prose 样式
            return marked.parse(text) as string;
        } catch (e) {
            return text;
        }
    }
</script>

<div class="w-full max-w-5xl mx-auto px-1 py-8 space-y-12">
    {#if !studyData}
        <!-- 空状态 -->
        <div class="flex flex-col items-center justify-center min-h-[400px] border-2 border-dashed border-muted rounded-xl bg-muted/10 p-12 text-center">
            <div class="bg-background p-4 rounded-full shadow-sm border border-border mb-6">
                <Sparkles class="w-8 h-8 text-primary" />
            </div>
            
            <h3 class="text-2xl font-bold tracking-tight mb-3 text-foreground">小皮书童智能分析</h3>
            <p class="text-muted-foreground max-w-md mb-8 leading-relaxed">
                让 AI 深度解析当前预设的结构逻辑、核心机制与改进建议，生成可视化蓝图与详细报告。
            </p>

            <Button
                onclick={generateStudy}
                disabled={loading}
                size="lg"
                class="font-medium px-8"
            >
                {#if loading}
                    <Loader2 class="w-4 h-4 mr-2 animate-spin" />
                    正在分析...
                {:else}
                    <Sparkles class="w-4 h-4 mr-2" />
                    开始生成分析报告
                {/if}
            </Button>
        </div>
    {:else}
        <!-- 头部区域 -->
        <header class="space-y-6 pb-8 border-b border-border">
            <div class="flex flex-col md:flex-row md:items-start md:justify-between gap-6">
                <div class="space-y-4 flex-1">
                    <!-- 标签组 -->
                    <div class="flex flex-wrap items-center gap-2">
                        {#if studyData.summary.architecture_type}
                            <Badge variant="outline" class="text-sm py-1 font-normal border-primary/20 text-primary bg-primary/5">
                                {studyData.summary.architecture_type}
                            </Badge>
                        {/if}
                        {#if studyData.summary.complexity_rating}
                            <Badge variant="secondary" class="text-sm py-1 font-normal">
                                {studyData.summary.complexity_rating}
                            </Badge>
                        {/if}
                        {#each studyData.summary.tags || [] as tag}
                            <Badge variant="outline" class="text-xs font-normal text-muted-foreground">
                                #{tag}
                            </Badge>
                        {/each}
                    </div>

                    <!-- 标题 -->
                    <h1 class="text-3xl font-extrabold tracking-tight text-foreground lg:text-4xl">
                        {studyData.summary.title || "预设分析报告"}
                    </h1>
                    
                    <!-- 一句话总结 -->
                    {#if studyData.summary.one_sentence_review}
                       <p class="text-lg text-muted-foreground leading-relaxed font-light border-l-4 border-primary pl-4 py-1">
                           {studyData.summary.one_sentence_review}
                       </p>
                    {/if}
                </div>

                <!-- 操作按钮 -->
                <div class="flex-shrink-0">
                    <Button variant="outline" onclick={generateStudy} disabled={loading} class="gap-2">
                         {#if loading}
                            <Loader2 class="w-4 h-4 animate-spin" />
                            重新生成中...
                        {:else}
                            <RefreshCw class="w-4 h-4" />
                            重新生成
                        {/if}
                    </Button>
                </div>
            </div>
        </header>

        <!-- 1. 结构蓝图 (单栏布局) -->
        <section class="space-y-6">
            <div class="flex items-center gap-3 mb-4">
                <div class="p-2 bg-primary/10 rounded-lg">
                    <Layers class="w-6 h-6 text-primary" />
                </div>
                <h2 class="text-2xl font-bold tracking-tight text-foreground">结构蓝图</h2>
            </div>
            
            <div class="rounded-xl border border-border bg-card shadow-sm overflow-hidden">
                <!-- Mermaid 图表容器 -->
                {#if studyData.structure_blueprint?.mermaid_code}
                    <div class="border-b border-border bg-muted/5 relative h-[600px] w-full overflow-hidden group">
                        <!-- 缩放控制按钮 -->
                        <div class="absolute bottom-4 right-4 z-10 flex flex-col gap-2 opacity-80 hover:opacity-100 transition-opacity">
                            <Button 
                                variant="outline" 
                                size="icon" 
                                class="h-8 w-8 bg-background/80 backdrop-blur" 
                                onclick={handleZoomIn}
                                aria-label="Zoom In"
                            >
                                <span class="text-lg font-bold">+</span>
                            </Button>
                            <Button 
                                variant="outline" 
                                size="icon" 
                                class="h-8 w-8 bg-background/80 backdrop-blur" 
                                onclick={handleZoomOut}
                                aria-label="Zoom Out"
                            >
                                <span class="text-lg font-bold">-</span>
                            </Button>
                            <Button 
                                variant="outline" 
                                size="icon" 
                                class="h-8 w-8 bg-background/80 backdrop-blur" 
                                onclick={handleResetZoom}
                                aria-label="Reset View"
                            >
                                <RefreshCw class="w-3 h-3" />
                            </Button>
                        </div>

                        <!-- 可缩放区域 -->
                        <div id="mermaid-wrapper" class="w-full h-full cursor-grab active:cursor-grabbing">
                             <div id="mermaid-container" class="mermaid-wireframe min-w-full min-h-full flex items-center justify-center">
                                <div class="flex items-center gap-2 text-muted-foreground p-12">
                                    <Loader2 class="w-5 h-5 animate-spin" />
                                    正在绘制结构图...
                                </div>
                            </div>
                        </div>
                    </div>
                {/if}

                <!-- 分析文本 -->
                <div class="p-8 space-y-8">
                    {#if studyData.structure_blueprint?.analysis}
                        <div class="space-y-3">
                            <h3 class="text-lg font-semibold text-foreground flex items-center gap-2">
                                设计逻辑分析
                            </h3>
                            <div class="prose prose-zinc dark:prose-invert max-w-none text-muted-foreground leading-relaxed">
                                {@html md(studyData.structure_blueprint.analysis)}
                            </div>
                        </div>
                    {/if}

                    {#if studyData.structure_blueprint?.pros_and_cons}
                         <Separator />
                         <div class="space-y-3">
                            <h3 class="text-lg font-semibold text-foreground flex items-center gap-2">
                                优缺点评估
                            </h3>
                            <div class="prose prose-zinc dark:prose-invert max-w-none text-muted-foreground leading-relaxed">
                                {@html md(studyData.structure_blueprint.pros_and_cons)}
                            </div>
                        </div>
                    {/if}
                </div>
            </div>
        </section>

        <!-- 2. 核心机制 (单栏) -->
        {#if studyData.mechanism_breakdown?.length > 0}
            <section class="space-y-6">
                <div class="flex items-center gap-3 mb-4">
                    <div class="p-2 bg-amber-500/10 rounded-lg">
                        <Zap class="w-6 h-6 text-amber-500" />
                    </div>
                    <h2 class="text-2xl font-bold tracking-tight text-foreground">核心机制解析</h2>
                </div>

                <div class="grid gap-6">
                    {#each studyData.mechanism_breakdown as mech, i}
                        <div class="group relative rounded-xl border border-border bg-card p-6 shadow-sm transition-shadow hover:shadow-md">
                            <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-4 mb-4 border-b border-border/50 pb-4">
                                <div class="flex items-center gap-3">
                                    <span class="flex items-center justify-center w-6 h-6 rounded-full bg-muted text-xs font-bold text-muted-foreground">
                                        {i + 1}
                                    </span>
                                    <h3 class="text-lg font-bold text-foreground">{mech.name}</h3>
                                </div>
                                {#if mech.source_identifier}
                                    <button
                                        class="inline-flex items-center gap-1.5 cursor-pointer hover:opacity-80 transition-opacity"
                                        onclick={() => openIdentifierPopover(mech.source_identifier)}
                                        title="点击查看条目内容"
                                    >
                                        <Badge variant="outline" class="font-mono text-xs border-primary/30 text-primary hover:bg-primary/5">
                                            <FileText class="w-3 h-3 mr-1" />
                                            {mech.source_identifier}
                                        </Badge>
                                    </button>
                                {/if}
                            </div>
                            
                            <div class="grid md:grid-cols-12 gap-6">
                                <div class="md:col-span-7 space-y-2">
                                    <h4 class="text-sm font-medium text-foreground/80 uppercase tracking-widest text-[10px]">工作原理</h4>
                                    <div class="prose prose-zinc dark:prose-invert text-sm text-muted-foreground max-w-none">
                                        {@html md(mech.how_it_works)}
                                    </div>
                                </div>
                                <div class="md:col-span-5 bg-muted/30 rounded-lg p-4 space-y-2">
                                    <h4 class="text-sm font-medium text-foreground/80 uppercase tracking-widest text-[10px]">关键作用</h4>
                                    <div class="prose prose-zinc dark:prose-invert text-sm text-muted-foreground max-w-none">
                                        {@html md(mech.why_it_matters)}
                                    </div>
                                </div>
                            </div>
                        </div>
                    {/each}
                </div>
            </section>
        {/if}

        <!-- 3. 缝合指南 (单栏) -->
         {#if studyData.stitching_guide}
            <section class="space-y-6">
                 <div class="flex items-center gap-3 mb-4">
                    <div class="p-2 bg-cyan-500/10 rounded-lg">
                        <Scissors class="w-6 h-6 text-cyan-500" />
                    </div>
                    <h2 class="text-2xl font-bold tracking-tight text-foreground">缝合指南</h2>
                </div>

                <div class="rounded-xl border border-border bg-card shadow-sm p-8 space-y-8">
                     {#if studyData.stitching_guide.description}
                        <div class="text-lg text-foreground/80 leading-relaxed font-medium">
                            {studyData.stitching_guide.description}
                        </div>
                    {/if}
                    
                    <div class="space-y-6">
                        {#each studyData.stitching_guide.recommendations || [] as rec}
                            <div class="flex gap-4 items-start">
                                <div class="mt-1 w-2 h-2 rounded-full bg-cyan-500 shadow-[0_0_8px_rgba(6,182,212,0.6)] shrink-0"></div>
                                <div class="space-y-2 flex-1">
                                    <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
                                        <h4 class="text-base font-bold text-foreground">{rec.module_type}</h4>
                                        {#if rec.suggested_position}
                                            <span class="text-xs font-mono text-cyan-600 dark:text-cyan-400 bg-cyan-100 dark:bg-cyan-950/30 px-2 py-0.5 rounded">
                                                建议位置：{rec.suggested_position}
                                            </span>
                                        {/if}
                                    </div>
                                    <div class="prose prose-zinc dark:prose-invert text-muted-foreground text-sm max-w-none">
                                        {@html md(rec.reasoning)}
                                    </div>
                                </div>
                            </div>
                        {/each}
                    </div>
                </div>
            </section>
        {/if}

        <!-- 4. 精彩片段与学习要点 (上下排列，不做双栏以保证宽度充足) -->
        <div class="space-y-10">
             {#if studyData.brilliant_snippets?.length > 0}
                <section class="space-y-6">
                    <div class="flex items-center gap-3 mb-4">
                        <div class="p-2 bg-yellow-500/10 rounded-lg">
                             <Lightbulb class="w-6 h-6 text-yellow-500" />
                        </div>
                        <h2 class="text-2xl font-bold tracking-tight text-foreground">高光片段</h2>
                    </div>
                    <div class="grid gap-4">
                        {#each studyData.brilliant_snippets as snippet}
                            <div class="bg-card border border-border rounded-lg p-6 space-y-4 shadow-sm hover:border-yellow-500/30 transition-colors">
                                <div class="flex items-center justify-between border-b border-border/50 pb-3">
                                    <Badge variant="outline" class="text-yellow-600 dark:text-yellow-400 border-yellow-500/20 bg-yellow-500/5">
                                        {snippet.technique || "技巧"}
                                    </Badge>
                                    {#if snippet.source_identifier}
                                        <button
                                            class="inline-flex items-center gap-1.5 cursor-pointer hover:opacity-80 transition-opacity"
                                            onclick={() => openIdentifierPopover(snippet.source_identifier)}
                                            title="点击查看条目内容"
                                        >
                                            <Badge variant="outline" class="font-mono text-xs border-primary/30 text-primary hover:bg-primary/5">
                                                <FileText class="w-3 h-3 mr-1" />
                                                {snippet.source_identifier}
                                            </Badge>
                                        </button>
                                    {/if}
                                </div>
                                {#if snippet.excerpt}
                                    <div class="bg-muted/30 p-4 rounded-md border-l-4 border-yellow-500/50 italic text-muted-foreground font-serif">
                                        "{snippet.excerpt}"
                                    </div>
                                {/if}
                                {#if snippet.analysis}
                                    <div class="prose prose-zinc dark:prose-invert text-sm text-foreground/90 max-w-none">
                                        {@html md(snippet.analysis)}
                                    </div>
                                {/if}
                            </div>
                        {/each}
                    </div>
                </section>
             {/if}

             {#if studyData.learning_points?.length > 0}
                <section class="space-y-6">
                    <div class="flex items-center gap-3 mb-4">
                        <div class="p-2 bg-rose-500/10 rounded-lg">
                            <BookOpen class="w-6 h-6 text-rose-500" />
                        </div>
                        <h2 class="text-2xl font-bold tracking-tight text-foreground">知识锦囊</h2>
                    </div>
                    <div class="bg-card border border-border rounded-xl p-8 shadow-sm">
                        <ul class="space-y-6">
                            {#each studyData.learning_points as point}
                                <li class="flex gap-4">
                                    <div class="flex flex-col items-center gap-1">
                                        <div class="w-8 h-8 rounded-full bg-rose-100 dark:bg-rose-900/20 flex items-center justify-center text-rose-600 dark:text-rose-400 font-bold text-sm">
                                            !
                                        </div>
                                        <div class="w-0.5 h-full bg-border/50 flex-1 my-1"></div>
                                    </div>
                                    <div class="space-y-2 pb-2">
                                        <h4 class="text-lg font-bold text-foreground">{point.concept}</h4>
                                        <div class="prose prose-zinc dark:prose-invert text-muted-foreground max-w-none">
                                            {@html md(point.actionable_lesson)}
                                        </div>
                                    </div>
                                </li>
                            {/each}
                        </ul>
                    </div>
                </section>
             {/if}
        </div>
    {/if}
</div>

<!-- 条目内容浮窗 -->
{#if popoverVisible}
    <!-- 背景遮罩 -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center p-4"
        onclick={closePopover}
    >
        <!-- 浮窗面板 -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
            class="bg-card border border-border rounded-xl shadow-2xl w-full max-w-3xl max-h-[80vh] flex flex-col overflow-hidden"
            onclick={(e) => e.stopPropagation()}
        >
            <!-- 标题栏 -->
            <div class="flex items-center justify-between px-6 py-4 border-b border-border bg-muted/30">
                <div class="flex items-center gap-3">
                    <FileText class="w-5 h-5 text-primary" />
                    <div>
                        <h3 class="text-lg font-bold text-foreground">{popoverName}</h3>
                        <span class="text-xs text-muted-foreground font-mono">{popoverIdentifier}</span>
                    </div>
                </div>
                <Button variant="ghost" size="icon" onclick={closePopover} class="h-8 w-8">
                    <X class="w-4 h-4" />
                </Button>
            </div>
            <!-- 内容区域 -->
            <div class="flex-1 overflow-auto p-6">
                <pre class="text-sm text-foreground whitespace-pre-wrap break-words font-mono leading-relaxed bg-muted/20 rounded-lg p-4 border border-border/50">{popoverContent}</pre>
            </div>
        </div>
    </div>
{/if}

<style>
    /* 强制 Mermaid 线框样式 */
    :global(.mermaid-wireframe svg) {
        max-width: none !important; /* 允许超出容器以便缩放 */
        height: auto !important;
        font-family: ui-sans-serif, system-ui, sans-serif !important;
    }
    
    /* 强力去除所有填充 */
    :global(.mermaid-wireframe .node rect),
    :global(.mermaid-wireframe .node circle),
    :global(.mermaid-wireframe .node polygon),
    :global(.mermaid-wireframe .node path),
    :global(.mermaid-wireframe .cluster rect),
    :global(.mermaid-wireframe rect),
    :global(.mermaid-wireframe circle),
    :global(.mermaid-wireframe path) {
        fill: transparent !important;
        stroke-width: 1.5px !important;
    }

    /* 文字颜色强制适配 */
    :global(.mermaid-wireframe text) {
        fill: currentColor !important; /* 跟随父级颜色 */
    }
    
    :global(.dark .mermaid-wireframe text) {
        fill: #e2e8f0 !important;
    }
    
     :global(:not(.dark) .mermaid-wireframe text) {
        fill: #020817 !important;
    }

    /* 边框颜色适配 */
    :global(.mermaid-wireframe .node rect),
    :global(.mermaid-wireframe .node circle),
    :global(.mermaid-wireframe .node polygon),
    :global(.mermaid-wireframe .cluster rect) {
        stroke: currentColor !important;
    }
    
    :global(.dark .mermaid-wireframe .node rect),
    :global(.dark .mermaid-wireframe .node circle),
    :global(.dark .mermaid-wireframe .node polygon) {
        stroke: #e2e8f0 !important;
    }
    
    :global(:not(.dark) .mermaid-wireframe .node rect),
    :global(:not(.dark) .mermaid-wireframe .node circle),
    :global(:not(.dark) .mermaid-wireframe .node polygon) {
        stroke: #020817 !important;
    }
</style>

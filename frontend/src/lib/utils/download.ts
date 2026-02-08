
import { isTauri, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile, writeFile, copyFile, remove } from '@tauri-apps/plugin-fs';
import { toast } from 'svelte-sonner';

interface DownloadProgress {
    downloaded: number;
    total: number;
    percent: number;
}

interface DownloadOptions {
    filename: string;
    content?: Blob | string | Uint8Array;
    url?: string;
    type?: string;
    fetchOptions?: RequestInit;
    /** 下载进度回调（仅对 URL 下载有效） */
    onProgress?: (percent: number, downloaded: number, total: number) => void;
    /** 是否使用流式下载（大文件推荐，默认 true） */
    useStreaming?: boolean;
}

/**
 * 统一的下载文件函数
 * 自动处理 Web 和 Tauri 环境下的下载逻辑
 * 
 * 对于大文件 URL 下载，使用流式下载到临时文件，避免内存溢出
 */
export async function downloadFile(options: DownloadOptions) {
    const { filename, content, url, type, fetchOptions, onProgress, useStreaming = true } = options;

    if (isTauri()) {
        try {
            // 1. 选择保存路径
            const filePath = await save({
                defaultPath: filename,
                filters: type ? [{
                    name: 'Files',
                    extensions: [filename.split('.').pop() || 'tmp']
                }] : undefined
            });

            if (!filePath) {
                // 用户取消
                return;
            }

            const toastId = toast.loading("正在保存...");

            // 2. 写入文件
            if (content) {
                // 处理内存内容（小文件直接写入）
                if (typeof content === 'string') {
                    await writeTextFile(filePath, content);
                } else if (content instanceof Blob) {
                    const buffer = await content.arrayBuffer();
                    await writeFile(filePath, new Uint8Array(buffer));
                } else if (content instanceof Uint8Array) {
                    await writeFile(filePath, content);
                }
            } else if (url) {
                // 从 fetchOptions 中提取 headers
                const headers = extractHeaders(fetchOptions);

                if (useStreaming) {
                    // 流式下载（推荐用于大文件）
                    await downloadWithStreaming(url, headers, filePath, onProgress);
                } else {
                    // 简单下载（兼容小文件）
                    await downloadSimple(url, headers, filePath);
                }
            }

            toast.dismiss(toastId);
            toast.success("保存成功", { description: `已保存到: ${filePath}` });

        } catch (e) {
            console.error("Save failed", e);
            toast.error("保存失败", { description: String(e) });
            throw e; // 重新抛出，让调用方可以处理
        }

    } else {
        // Web 浏览器环境
        await downloadForWeb(options);
    }
}

/**
 * 从 fetchOptions 提取 headers
 */
function extractHeaders(fetchOptions?: RequestInit): Record<string, string> | undefined {
    if (!fetchOptions?.headers) return undefined;

    let headers: Record<string, string> = {};

    if (fetchOptions.headers instanceof Headers) {
        fetchOptions.headers.forEach((value, key) => {
            headers[key] = value;
        });
    } else if (Array.isArray(fetchOptions.headers)) {
        fetchOptions.headers.forEach(([key, value]) => {
            headers[key] = value;
        });
    } else {
        headers = fetchOptions.headers as Record<string, string>;
    }

    return Object.keys(headers).length > 0 ? headers : undefined;
}

/**
 * 流式下载（大文件，带进度）
 * 使用 Rust 后端流式写入临时文件，然后复制到目标路径
 * 兼容 Android content:// URI
 */
async function downloadWithStreaming(
    url: string,
    headers: Record<string, string> | undefined,
    targetPath: string,
    onProgress?: (percent: number, downloaded: number, total: number) => void
): Promise<void> {
    // 使用顶层静态导入的 invoke 和 listen

    // 监听进度事件
    let unlisten: UnlistenFn | undefined;
    if (onProgress) {
        unlisten = await listen<DownloadProgress>('download-progress', (event) => {
            onProgress(event.payload.percent, event.payload.downloaded, event.payload.total);
        });
    }

    let tempPath: string | null = null;

    try {
        // 调用 Rust 命令流式下载到临时文件
        tempPath = await invoke<string>('download_with_progress', {
            url,
            headers
        });

        // 复制到用户选择的目标路径（兼容 Android content:// URI）
        await copyFile(tempPath, targetPath);

    } finally {
        // 清理：停止监听
        if (unlisten) {
            unlisten();
        }

        // 清理：删除临时文件
        if (tempPath) {
            try {
                await remove(tempPath);
            } catch (e) {
                console.warn("Failed to remove temp file:", e);
            }
        }
    }
}

/**
 * 简单下载（小文件，直接返回字节数组）
 */
async function downloadSimple(
    url: string,
    headers: Record<string, string> | undefined,
    targetPath: string
): Promise<void> {
    // 使用顶层静态导入的 invoke

    // 调用 Rust 命令下载，返回字节数组
    const data = await invoke<number[]>('download_large_file', {
        url,
        headers
    });

    // 写入目标路径
    await writeFile(targetPath, new Uint8Array(data));
}

/**
 * Web 浏览器环境下载
 */
async function downloadForWeb(options: DownloadOptions): Promise<void> {
    const { filename, content, url, type, fetchOptions } = options;

    let downloadUrl = url;
    let shouldRevoke = false;

    if (content) {
        let blob: Blob;
        if (content instanceof Blob) {
            blob = content;
        } else if (content instanceof Uint8Array) {
            // @ts-expect-error SharedArrayBuffer mismatch in TS definition
            blob = new Blob([content], { type: type || 'application/octet-stream' });
        } else {
            // string
            const mime = type || (optionHasJson(options) ? "application/json" : "text/plain");
            blob = new Blob([content], { type: mime });
        }

        downloadUrl = window.URL.createObjectURL(blob);
        shouldRevoke = true;
    } else if (url && fetchOptions) {
        // Web 环境需 fetch blob 以支持 Header
        try {
            const response = await fetch(url, fetchOptions);
            if (!response.ok) throw new Error("Export failed");
            const blob = await response.blob();
            downloadUrl = window.URL.createObjectURL(blob);
            shouldRevoke = true;
        } catch (e) {
            console.error("Web fetch download failed", e);
            toast.error("下载失败");
            return;
        }
    }

    if (!downloadUrl) return;

    const a = document.createElement("a");
    a.href = downloadUrl;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);

    if (shouldRevoke) {
        setTimeout(() => window.URL.revokeObjectURL(downloadUrl!), 100);
    }

    if (!content && !fetchOptions) {
        toast.info("已触发下载", { description: "请查看浏览器下载记录" });
    }
}

function optionHasJson(opt: DownloadOptions) {
    if (opt.filename.endsWith(".json")) return true;
    if (typeof opt.content === 'string') {
        try {
            JSON.parse(opt.content);
            return true;
        } catch { }
    }
    return false;
}

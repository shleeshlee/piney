
import { isTauri, invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile, writeFile } from '@tauri-apps/plugin-fs';
import { platform } from '@tauri-apps/plugin-os';
import { toast } from 'svelte-sonner';

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

// Android 公共存储导出目录
const ANDROID_EXPORT_DIR = '/storage/emulated/0/Download/Piney';

/**
 * 获取 Android 导出路径
 * 目录创建由 Rust 端 write_to_android_public 命令处理
 */
function getAndroidExportPath(filename: string): string {
    return `${ANDROID_EXPORT_DIR}/${filename}`;
}

/**
 * 统一的下载文件函数
 * 自动处理 Web 和 Tauri 环境下的下载逻辑
 * 
 * - Android：直接保存到 Download/Piney/ 目录
 * - Win/Mac/Linux：弹出保存对话框让用户选择位置
 * 
 * 对于大文件 URL 下载，使用流式下载到临时文件，避免内存溢出
 */
export async function downloadFile(options: DownloadOptions) {
    const { filename, content, url, type, fetchOptions, onProgress, useStreaming = true } = options;

    if (isTauri()) {
        try {
            const currentPlatform = platform();
            let filePath: string;

            // 根据平台选择保存方式
            if (currentPlatform === 'android') {
                // Android：直接使用公共存储目录
                filePath = getAndroidExportPath(filename);
            } else {
                // Win/Mac/Linux：弹出保存对话框
                const selectedPath = await save({
                    defaultPath: filename,
                    filters: type ? [{
                        name: 'Files',
                        extensions: [filename.split('.').pop() || 'tmp']
                    }] : undefined
                });

                if (!selectedPath) {
                    // 用户取消
                    return;
                }
                filePath = selectedPath;
            }

            const toastId = toast.loading("正在保存...");

            // 写入文件
            if (content) {
                // 处理内存内容
                let dataBytes: Uint8Array;
                if (typeof content === 'string') {
                    dataBytes = new TextEncoder().encode(content);
                } else if (content instanceof Blob) {
                    const buffer = await content.arrayBuffer();
                    dataBytes = new Uint8Array(buffer);
                } else {
                    dataBytes = content;
                }

                // Android 使用专用写入命令（支持扫描更新文件大小显示）
                if (currentPlatform === 'android') {
                    await invoke('write_to_android_public', {
                        targetPath: filePath,
                        data: Array.from(dataBytes)
                    });
                } else {
                    // 非 Android 使用 plugin-fs
                    if (typeof content === 'string') {
                        await writeTextFile(filePath, content);
                    } else {
                        await writeFile(filePath, dataBytes);
                    }
                }
            } else if (url) {
                // 从 fetchOptions 中提取 headers、method、body
                const headers = extractHeaders(fetchOptions);
                const method = fetchOptions?.method as string | undefined;
                const body = typeof fetchOptions?.body === 'string' ? fetchOptions.body : undefined;

                if (useStreaming) {
                    // 流式下载（推荐用于大文件）
                    await downloadWithStreaming(url, headers, filePath, method, body);
                } else {
                    // 简单下载（兼容小文件）
                    await downloadSimple(url, headers, filePath, method, body);
                }
            }

            toast.dismiss(toastId);

            // Android 显示更友好的提示
            if (currentPlatform === 'android') {
                toast.success("保存成功", { description: `已保存到: Download/Piney/${filename}` });
            } else {
                toast.success("保存成功", { description: `已保存到: ${filePath}` });
            }

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
 * 流式下载（大文件）
 * Rust 后端流式下载到临时文件，然后复制到目标路径
 * 支持 Android content:// URI（由 Rust 端处理）
 * 支持 POST/PUT 等带请求体的方法
 */
async function downloadWithStreaming(
    url: string,
    headers: Record<string, string> | undefined,
    targetPath: string,
    method?: string,
    body?: string
): Promise<void> {
    await invoke<void>('download_with_progress', {
        url,
        headers,
        targetPath,
        method,
        body
    });
}

/**
 * 简单下载（小文件，直接返回字节数组）
 */
async function downloadSimple(
    url: string,
    headers: Record<string, string> | undefined,
    targetPath: string,
    method?: string,
    body?: string
): Promise<void> {
    // 调用 Rust 命令下载，返回字节数组
    const data = await invoke<number[]>('download_large_file', {
        url,
        headers,
        method,
        body
    });

    // 根据平台选择写入方式
    const currentPlatform = platform();
    if (currentPlatform === 'android') {
        // Android 使用专用命令（支持扫描更新文件大小）
        await invoke('write_to_android_public', {
            targetPath,
            data
        });
    } else {
        await writeFile(targetPath, new Uint8Array(data));
    }
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

// The only place that talks to /api. Mutations carry the session header the server requires.

import { boot } from '@/boot'

export class ApiError extends Error {
    status: number
    constructor(status: number, message: string) {
        super(message)
        this.status = status
    }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const res = await fetch(path, {
        method,
        headers: {
            Accept: 'application/json',
            ...(body !== undefined ? { 'Content-Type': 'application/json' } : {}),
            ...(method !== 'GET' ? { 'X-Switcheroo-Session': boot.session } : {}),
        },
        body: body !== undefined ? JSON.stringify(body) : undefined,
    })
    const text = await res.text()
    let data: unknown = null
    try {
        data = text ? JSON.parse(text) : null
    } catch {
        data = text
    }
    if (!res.ok) {
        const msg =
            data && typeof data === 'object' && 'error' in data
                ? String((data as { error: unknown }).error)
                : res.statusText
        throw new ApiError(res.status, msg)
    }
    return data as T
}

export const api = {
    get: <T>(path: string) => request<T>('GET', path),
    post: <T>(path: string, body?: unknown) => request<T>('POST', path, body ?? {}),
    patch: <T>(path: string, body: unknown) => request<T>('PATCH', path, body),
    put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
    del: <T>(path: string) => request<T>('DELETE', path),
}

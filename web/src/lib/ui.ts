import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs))
}

/** Throws with `message` when `condition` is falsy, and narrows the type otherwise. */
export function invariant(condition: unknown, message: string): asserts condition {
    if (!condition) {
        throw new Error(message)
    }
}

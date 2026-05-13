export function compareVersionNumbers(left: string, right: string) {
    const toParts = (value: string) => value
        .trim()
        .replace(/^[^\d]*/, "")
        .split(/[^0-9]+/)
        .filter(Boolean)
        .map((part) => Number.parseInt(part, 10));
    const leftParts = toParts(left);
    const rightParts = toParts(right);
    const maxLength = Math.max(leftParts.length, rightParts.length);
    for (let index = 0; index < maxLength; index += 1) {
        const leftPart = leftParts[index] ?? 0;
        const rightPart = rightParts[index] ?? 0;
        if (leftPart > rightPart) {
            return 1;
        }
        if (leftPart < rightPart) {
            return -1;
        }
    }
    return 0;
}

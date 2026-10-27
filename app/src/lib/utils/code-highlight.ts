export type CodeLanguage = "shell" | "python" | "javascript" | "ini";

const COMMENT_PATTERN: Record<CodeLanguage, string> = {
    shell: "#.*",
    python: "#.*",
    ini: "#.*",
    javascript: "//.*",
};

const KEYWORDS: Record<CodeLanguage, ReadonlySet<string>> = {
    shell: new Set(["export", "aws", "rclone"]),
    python: new Set(["import", "from", "as", "for", "in", "print"]),
    javascript: new Set(["import", "from", "const", "await", "new", "for", "of", "console"]),
    ini: new Set(),
};

const STRING_PATTERN = `"(?:[^"\\\\\\n]|\\\\.)*"|'(?:[^'\\\\\\n]|\\\\.)*'`;

function escapeHtml(text: string): string {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function wrap(className: string, text: string): string {
    return `<span class="${className}">${text}</span>`;
}

export function highlightCode(code: string, language: CodeLanguage): string {
    const keywords = KEYWORDS[language];
    const pattern = new RegExp(
        `(${COMMENT_PATTERN[language]})|(${STRING_PATTERN})|(\\b\\d+(?:\\.\\d+)?\\b)|([A-Za-z_][\\w]*)`,
        "gm",
    );
    return escapeHtml(code).replace(
        pattern,
        (match: string, comment?: string, string?: string, number?: string, word?: string, offset = 0, source = "") => {
            if (comment) return wrap("syn-comment", match);
            if (string) return wrap("syn-string", match);
            if (number) return wrap("syn-number", match);
            if (word && keywords.has(word)) return wrap("syn-keyword", match);
            return word && /^[ \t]*=(?!=)/.test(source.slice(offset + match.length)) ? wrap("syn-key", match) : match;
        },
    );
}

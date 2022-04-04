declare global {
  var kvStorage: {
    set: (key: string, value: string) => Promise<void>,
    get: (key: string) => Promise<string>,
    delete: (key: string) => Promise<void>,
    clear: () => Promise<void>,
    keys: () => Promise<string[]>,
    values: () => Promise<string[]>,
    entries: () => Promise<[string, string][]>,
  }
}

export { };
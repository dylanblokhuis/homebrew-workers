"use strict";

((window) => {
  const core = window.Deno.core;

  /**
   * @param {string} key 
   * @param {string} value 
   * 
   * @returns {Promise<void>}
   */
  function set(key, value) {
    return core.opAsync("op_kv_set", key, value);
  }

  /**
   * @param {string} key 
   * 
   * @returns {Promise<string | null>}
   */
  function get(key) {
    return core.opAsync("op_kv_get", key);
  }

  /**
   * @param {string} key 
   * 
   * @returns {Promise<void>}
   */
  function _delete(key) {
    return core.opAsync("op_kv_delete", key);
  }

  /**
   * @returns {Promise<void>}
   */
  function clear() {
    return core.opAsync("op_kv_clear");
  }

  window.kvStorage = {
    set,
    get,
    "delete": _delete,
    clear,
    keys: async function () {
      const all = await core.opAsync("op_kv_all");
      return Object.keys(all);
    },
    values: async function () {
      const all = await core.opAsync("op_kv_all");
      return Object.values(all);
    },
    entries: async function () {
      const all = await core.opAsync("op_kv_all");
      return Object.entries(all);
    },
  };
})(this);

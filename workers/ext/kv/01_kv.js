"use strict";

((window) => {
  const core = window.Deno.core;

  /**
   * @param {string} name 
   * @param {string} value 
   * 
   * @returns {Promise<void>}
   */
  function set(name, value) {
    return core.opAsync("op_kv_set", name, value);
  }

  /**
   * @param {string} name 
   * 
   * @returns {Promise<string>}
   */
  function get(name) {
    return core.opAsync("op_kv_get", name);
  }

  window.kvStorage = {
    set,
    get
  };
})(this);

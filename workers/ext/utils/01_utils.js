"use strict";

((window) => {
  /**
   * @param {Response} response 
   * @returns {Promise<void}
   */
  async function respondWith(response) {
    const serialized = {
      headers: Object.fromEntries(response.headers),
      ok: response.ok,
      redirected: response.redirected,
      status: response.status,
      statusText: response.statusText,
      trailer: response.trailer,
      type: response.type,
      body: new Uint8Array(await response.arrayBuffer())
    }

    window.requestResult = serialized
  }

  /**
   * @param {any} request 
   * @returns {Promise<any>}
   */
  async function callOnRequest(request) {
    const event = {
      request: new Request(request.url, {
        method: request.method,
        headers: request.headers,
        body: (request.method !== "GET" && request.method !== "HEAD") ? request.body : undefined
      }),
      respondWith: respondWith
    }

    return window.onRequest(event)
  }

  window.callOnRequest = callOnRequest
})(this);

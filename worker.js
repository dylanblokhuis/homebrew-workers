async function onRequest(event) {
  const response = new Response("Test", {
    headers: {
      "X-Proto": "Test"
    }
  });

  event.respondWith({
    headers: Object.fromEntries(response.headers),
    ok: response.ok,
    redirected: response.redirected,
    status: response.status,
    statusText: response.statusText,
    trailer: response.trailer,
    type: response.type,
    body: await response.text()
  })
}
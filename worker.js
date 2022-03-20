async function onRequest(event) {
  const response = new Response("Test", {
    headers: {
      "X-Proto": "Test"
    }
  });

  event.respondWith(response)
}
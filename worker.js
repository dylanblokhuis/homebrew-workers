async function onRequest(request) {
  console.log("onRequest", request);

  const response = new Response("Test", {
    headers: {
      "X-Proto": "Test"
    }
  });

  // const text = await Deno.readTextFile("./some-app/main.js");
  // console.log(text);
  // await new Promise(r => setTimeout(r, 1000));

  return {
    headers: Object.fromEntries(response.headers),
    ok: response.ok,
    redirected: response.redirected,
    status: response.status,
    statusText: response.statusText,
    trailer: response.trailer,
    type: response.type,
    body: await response.text()
  };
}
const { uuidv7 } = require("uuidv7");
const { encode, decode } = require("@msgpack/msgpack");

const api = process.env.API;
const password = process.env.PASSWORD;
const lag = 75;

const responses = [];

async function send(method, path, data) {
  const request = {
    method,
    headers: { "Content-Type": "application/msgpack", Authorization: password ? "Bearer " + password : undefined },
    body: data === undefined ? undefined : encode(data),
  };
  const startTime = performance.now();
  const response = await fetch(api + path, request);
  const ping = performance.now() - startTime;
  responses.push(decode(await response.arrayBuffer()));
  console.log(`[${ping.toFixed(2).padStart(6, "0")} ms]`, `${method} ${path}`, data, responses.at(-1));
}

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

let roomName = "";
let alice = "";
let bob = "";

(async () => {
  for (let i = 0; i < 40; i++) {
    await send("GET", "/status");
    await wait(1000);
  }
})().catch(console.error);

(async () => {
  await send("POST", "/room/create", { version: "0.1", user: { name: "Alice" }, size: 4 });
  const session = responses.at(-1).session_id;
  alice = responses.at(-1).user_id;
  const room = responses.at(-1).id;
  roomName = responses.at(-1).name;
  for (let i = 0; i < 20; i++) {
    const reports = [];
    const actions = [];
    for (let j = 0; j < 2; j++) {
      reports.push({ id: uuidv7(), type: "hoge", event: `${i}.${j}` });
      actions.push({ id: uuidv7(), type: "hoge", event: `${i}.${j}` });
      await wait(50 + Math.random() * lag);
    }
    await send("POST", "/room/sync", { session_id: session, room_info: i, reports, actions });
  }
})().catch(console.error);

(async () => {
  while (!roomName) await wait(50);
  await wait(1000);
  await send("POST", "/room/join", { version: "0.1", name: roomName, user: { name: "Bob" } });
  const session = responses.at(-1).session_id;
  const room = responses.at(-1).id;
  bob = responses.at(-1).user_id;
  for (let i = responses.at(-1).room_info + 1; i < 60; i++) {
    const reports = [];
    const actions = [];
    for (let j = 0; j < 2; j++) {
      reports.push({ id: uuidv7(), type: "fuga", event: `${i}.${j}` });
      actions.push({ id: uuidv7(), type: "fuga", event: `${i}.${j}` });
      await wait(50 + Math.random() * lag);
    }
    await send("POST", "/room/sync", { session_id: session, room_info: i, reports, actions });
  }
})().catch(console.error);

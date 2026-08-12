const form = document.querySelector("#feedback-form");
const statusOutput = document.querySelector("#form-status");
const commentsList = document.querySelector("#comments-list");
const refreshButton = document.querySelector("#refresh-comments");
const copyDonationButton = document.querySelector("#copy-donation");

const kindLabels = { bug: "Ошибка", idea: "Улучшение", wish: "Пожелание", comment: "Комментарий" };

function formatDate(value) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : new Intl.DateTimeFormat("ru-RU", { day: "2-digit", month: "short", year: "numeric" }).format(date);
}

function renderComments(items) {
  commentsList.replaceChildren();
  if (!items.length) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "Пока тихо. Оставьте первое содержательное сообщение.";
    commentsList.append(empty);
    return;
  }
  items.forEach((item) => {
    const article = document.createElement("article");
    article.className = "comment";
    const head = document.createElement("div");
    head.className = "comment-head";
    const author = document.createElement("strong");
    author.textContent = item.display_name;
    const meta = document.createElement("span");
    meta.textContent = formatDate(item.created_at);
    const kind = document.createElement("span");
    kind.className = "comment-kind";
    kind.textContent = kindLabels[item.kind] || "Комментарий";
    const body = document.createElement("p");
    body.textContent = item.body;
    head.append(author, meta);
    article.append(head, kind, body);
    commentsList.append(article);
  });
}

async function loadComments() {
  refreshButton.disabled = true;
  try {
    const response = await fetch("/api/comments?limit=24", { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error("comments unavailable");
    const payload = await response.json();
    renderComments(Array.isArray(payload.items) ? payload.items : []);
  } catch {
    commentsList.innerHTML = '<p class="empty-state">Не удалось загрузить сообщения. Попробуйте обновить позже.</p>';
  } finally {
    refreshButton.disabled = false;
  }
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const submitButton = form.querySelector('button[type="submit"]');
  const values = new FormData(form);
  statusOutput.className = "";
  statusOutput.textContent = "Проверяем сообщение…";
  submitButton.disabled = true;
  try {
    const response = await fetch("/api/comments", {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(Object.fromEntries(values.entries())),
    });
    const payload = await response.json();
    if (!response.ok && response.status !== 202) throw new Error(payload.error || "submit_failed");
    statusOutput.textContent = payload.message || "Спасибо. Сообщение принято.";
    form.reset();
    if (payload.status === "approved") await loadComments();
  } catch (error) {
    statusOutput.className = "error";
    statusOutput.textContent = error.message === "rate_limited" ? "Слишком много сообщений. Попробуйте через несколько минут." : "Не удалось отправить сообщение. Проверьте соединение и попробуйте ещё раз.";
  } finally {
    submitButton.disabled = false;
  }
});

refreshButton.addEventListener("click", loadComments);
copyDonationButton.addEventListener("click", async () => {
  const number = document.querySelector("#donation-number").textContent.replaceAll(" ", "");
  try {
    await navigator.clipboard.writeText(number);
    copyDonationButton.textContent = "Скопировано";
    window.setTimeout(() => { copyDonationButton.textContent = "Скопировать номер"; }, 1800);
  } catch {
    copyDonationButton.textContent = "Выделите номер выше";
  }
});

loadComments();

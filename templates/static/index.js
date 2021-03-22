const cookies = () => document.cookie
    .split(";")
    .reduce((o,c) => ({...o, [/[^ =]+/.exec(c)[0].trim()]: /(?<=\=).+/.exec(c)[0]}), {});


const form = document.querySelector("form.sender");
const form_text = document.querySelector("form.sender input[name=text]");
const statusbar = document.getElementById("form-status-bar");
const submit = document.querySelector("form.sender input[type=submit]");
const r_checks = Array.from(document.querySelectorAll("form.sender input[type=checkbox].recipient-checkbox"));
const comma = document.createTextNode(",");
const messages = document.querySelector(".messages");
const bracket = document.createTextNode("]");

statusbar.style.color = "darkslategrey";
statusbar.textContent = "rendering...";
let status_bar_wellformed = false;

// =============================================================================
// TO SHOW ERRORS

const showError = err => {
    let msg = document.createElement("span");
    msg.style.color = "red";
    msg.style.cursor = "pointer";
    msg.innerText = "!error!"
    msg.onclick = () => {
        let pre = document.createElement("pre");
        pre.textContent = err + "\n===\n" + JSON.stringify(err, null, 2);
        document.body.innerHTML = '';
        document.body.appendChild(pre);
    }
    statusbar.innerHTML = ''
    statusbar.replaceChildren(msg);

}

// =============================================================================
// UPDATE STATUS BAR

let r_arr_marks = {};
const onCheckClick = (e) => {
  if (status_bar_wellformed) {
    if (e.target.checked) {
      if ( statusbar.textContent == "> []") statusbar.textContent = "> [";

      let label = r_arr_marks[e.target.id];
      if (!label) {
        const acronyms = e.target.parentElement.getElementsByClassName("acronym");
        if (acronyms.length > 0) {
          label = acronyms[0].cloneNode(true);
          r_arr_marks[e.target.id] = label;
        }
      }
      if (statusbar.contains(bracket)) statusbar.removeChild(bracket);
      if (statusbar.lastChild.nodeType != 3) {
        last_comma = comma.cloneNode(true);
        statusbar.appendChild(last_comma);
      }
      statusbar.appendChild(label);
      statusbar.appendChild(bracket);
    } else {
      let label = r_arr_marks[e.target.id];
      let prev = label.previousSibling;
      let next = label.nextSibling;
      if (statusbar.contains(label)) statusbar.removeChild(label);
      if (prev && prev.textContent == ",") statusbar.removeChild(prev);
      else {
        if (next && next.textContent == ",") statusbar.removeChild(next);
      }

    }
  } else {
    renderStatusBar();
  }
};


// =============================================================================
// RENDER STATUS BAR

const renderStatusBar = () => {
  let is_empty = true;
  statusbar.textContent = "> [";

  let last_comma = null;
  for (let i = 0; i < r_checks.length; i++) {
    let each = r_checks[i];
    if (each.checked) {
      is_empty = false;
      let label = r_arr_marks[each.id];
      if (!label) {
        const acronyms = each.parentElement.getElementsByClassName("acronym");
        if (acronyms.length > 0) {
          label = acronyms[0].cloneNode(true);
          r_arr_marks[each.id] = label;
        }
      }
      statusbar.appendChild(label);
      last_comma = comma.cloneNode(true);
      comma_stack.push(last_comma);
      statusbar.appendChild(last_comma);
    }
  }
  if (is_empty) {
    statusbar.textContent = "> []";
  } else {
    statusbar.removeChild(last_comma)  
    statusbar.appendChild(bracket);
  }
  status_bar_wellformed = true;
};

r_checks.forEach(a => a.addEventListener("change", onCheckClick));
renderStatusBar();

// =============================================================================
// WEB SOCKETS

let socket = new WebSocket("ws://"+window.location.host+"/websocket"); 
socket.onopen = e => socket.send(cookies().token); 
socket.onclose = (e) => {
    alert("Websocket closed");
    console.error(e);
}
socket.onmessage = e => {
    let data = JSON.parse(e.data);
    let text_part = document.createElement("div");
    text_part.className = "text";
    text_part.textContent = data.text;

    let head_part = document.createElement("div");
    head_part.className = "head";

    let sender_span = document.createElement("span");
    sender_span.title = data.sender_name;
    sender_span.textContent = data.sender_name.match(/[A-ZА-Я]/g).reduce((a,b) => a+b);
    sender_span.style.color = data.sender_color;

    head_part.appendChild(sender_span);
    head_part.appendChild(document.createTextNode(" -> [ "));

    data.recipients.forEach(each => {
        let r_span = document.createElement("span");
        r_span.title = each.name;
        r_span.textContent = each.name.match(/[A-ZА-Я]/g).reduce((a,b) => a+b);
        r_span.style.color = each.color;
        head_part.appendChild(r_span);
        head_part.appendChild(document.createTextNode(" "));
    });

    head_part.appendChild(document.createTextNode("]"));

    let time = new Date(data.timestamp*1000);
    console.log(time);
    let time_span = document.createElement("span");
    time_span.title = "" + time.getYear() + "-"
            + time.getMonth() + "-"
            + time.getDay() + " "
            + time.getHours() + ":"
            + time.getMinutes() + ":"
            + time.getSeconds();
    time_span.textContent = "" + time.getHours() + ":" + time.getMinutes();
    time_span.className = "time";
    head_part.appendChild(time_span);
    

    let msg_part = document.createElement("div");
    msg_part.className = "message";
    msg_part.appendChild(head_part);
    msg_part.appendChild(text_part);
    messages.appendChild(msg_part);
}

// =============================================================================
// AJAX POST FORM

form.addEventListener("submit", async e => {
    e.preventDefault();
    const formData = new FormData(form);

    await fetch(window.location.origin, {
        method: "POST",
        body: formData,
        credentials: 'same-origin',
    }).then(s => {
        if (s.status == 200) form.reset();
        else showError(s); 
    }).catch(e => {
        showError(e)
    });

});

// =============================================================================
// AUTO CLOSE POPUP CONTAINERS

window.addEventListener("click", (e) => {
    for (i of e.path) { 
        if (i.className && i.className.includes("popup-container")) { return; }
    }
    for (c of document.querySelectorAll(".popup-container input.popup-trigger")) {
        c.checked = false;
    }
});

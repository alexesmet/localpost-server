# LocalPost Server
LocalPost server is a simplest chatting web application. Motto: _Best code is no code_.
I tried to rely on simplest techologies possible.

![screenshot](./screenshot.png?raw=true "How it looks")

LocalPost server renders a page with a templating engine. Resulting page works without
javascript, but enabling javascript allows you to see new messages without reloading 
the page.

The biggest challange for me was to implement multipart form data to allow file 
uploading, since this feature was not implemented in Tide at the time of writing.

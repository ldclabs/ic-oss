<script>
  let greeting = "";

  function onSubmit(event) {
    return false;
  }
</script>

<main>
  <img src="/logo2.svg" alt="DFINITY logo" />
  <br />
  <br />
  <form action="#" on:submit|preventDefault={onSubmit}>
    <label for="name">Enter your name: &nbsp;</label>
    <input id="name" alt="Name" type="text" />
    <button type="submit">Click Me!</button>
  </form>
  <section id="greeting">{greeting}</section>
</main>

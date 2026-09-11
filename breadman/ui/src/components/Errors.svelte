<script lang="ts">
  import { errors } from "../lib/stores";
</script>

<div class="form-view">
  <div class="settings">
    <h1>Errors</h1>
    {#if $errors.length === 0}
      <p class="err-empty">No errors or warnings this session.</p>
    {:else}
      <div class="err-list">
        {#each [...$errors].reverse() as e, i (i)}
          <div class="err-row">
            <time>{e.time}</time>
            <span>{e.message}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

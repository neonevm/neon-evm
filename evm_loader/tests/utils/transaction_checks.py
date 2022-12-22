import base64

from ..solana_utils import solana_client


def check_transaction_logs_have_text(trx_hash, text):

    receipt = solana_client.get_transaction(trx_hash)
    logs = ""
    for log in receipt.value.transaction.meta.log_messages:
        if "Program data:" in log:
            logs += "Program data: " + str(base64.b64decode(log.replace("Program data: ", "")))
        else:
            logs += log
        logs += " "
    assert text in logs, f"Transaction logs don't contain '{text}'. Logs: {logs}"


def check_holder_account_tag(storage_account, layout, expected_tag):
    account_data = solana_client.get_account_info(storage_account).value.data
    parsed_data = layout.parse(account_data)
    assert parsed_data.tag == expected_tag, f"Account data {account_data} doesn't contain tag {expected_tag}"


import click
import requests
import os


class GithubClient():

    def __init__(self, token):
        self.proxy_endpoint = os.environ.get("PROXY_ENDPOINT")
        self.headers = {"Authorization": f"Bearer {token}",
                        "Accept": "application/vnd.github+json"}

    def get_proxy_runs_list(self, proxy_branch):
        response = requests.get(
            f"{self.proxy_endpoint}/actions/workflows/pipeline.yml/runs?branch={proxy_branch}", headers=self.headers)
        if response.status_code != 200:
            raise RuntimeError(f"Can't get proxy runs list. Error: {response.json()}")
        runs = [item['id'] for item in response.json()['workflow_runs']]
        return runs

    def get_proxy_runs_count(self, proxy_branch):
        response = requests.get(
            f"{self.proxy_endpoint}/actions/workflows/pipeline.yml/runs?branch={proxy_branch}", headers=self.headers)
        return int(response.json()["total_count"])

    def run_proxy_dispatches(self, proxy_branch, github_ref, github_sha, test_set, initial_pr):
        data = {"ref": proxy_branch,
                "inputs": {"test_set": test_set,
                           "neon_evm_commit": github_sha,
                           "neon_evm_branch": github_ref,
                           "initial_pr": initial_pr}
                }
        response = requests.post(
            f"{self.proxy_endpoint}/actions/workflows/pipeline.yml/dispatches", json=data, headers=self.headers)
        click.echo(f"Sent data: {data}")
        click.echo(f"Status code: {response.status_code}")
        if response.status_code != 204:
            raise RuntimeError("proxy-model.py action is not triggered")

    @staticmethod
    def get_branches_list(endpoint):
        proxy_branches_obj = requests.get(
            f"{endpoint}/branches?per_page=100").json()
        return [item["name"] for item in proxy_branches_obj]
    @staticmethod
    def is_branch_exist(endpoint, branch):
        if branch:
            response = requests.get(f"{endpoint}/branches/{branch}")
            if response.status_code == 200:
                click.echo(f"The branch {branch} exist in the {endpoint} repository")
                return True
        else:
            return False

    def get_proxy_run_info(self, id):
        response = requests.get(
            f"{self.proxy_endpoint}/actions/runs/{id}", headers=self.headers)
        return response.json()
